//! D67: a handler of the page that says nothing of a failure is named here, with why. `window.js` is
//! read by a JavaScript parser: a reading of its text was shown blind twice (#594, #595).

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

/// The handlers that stay silent: what they answer — a command, or the first thing their `try` or
/// promise calls —, how many times, and why.
const SILENT: &[(&str, usize, &str)] = &[
    ("page_failed", 1, "the log itself: nowhere left to say it"),
    ("makings", 2, "returns no `Result`: it cannot fail"),
    ("header", 1, "returns no `Result`: it cannot fail"),
    ("recording_now", 1, "returns no `Result`: it cannot fail"),
    (
        "model_fetch",
        2,
        "returns no `Result`: how the fetch goes is said where it is shown",
    ),
    (
        "ollama_pull_offered",
        1,
        "returns no `Result`: how the fetch goes is said where it is shown",
    ),
    ("asked", 1, "model_offered_fetch or ollama_pull_offered: neither returns a `Result`"),
    (
        "registry_refresh",
        1,
        "`.then(() => true, () => false)`: the answer is said by the lines after it",
    ),
    (
        "speech_installed",
        1,
        "`.then(() => true, () => false)`: the answer is said by the lines after it; the assistant's logs it",
    ),
    (
        "listed",
        1,
        "the small window's rest: fails only if its task dies, logged by `told`; asked again at the next tick",
    ),
    (
        "speech_shown",
        2,
        "an offer: returns `Ok` on what it reads, and is simply absent otherwise",
    ),
    (
        "context_shown",
        2,
        "a meeting gone, logged by `told`; it answers no gesture: its flag and its text stay as they were",
    ),
    (
        "opened",
        1,
        "a meeting gone, logged by `told`: most often sent to the trash here, and the list shows it",
    ),
    ("URL", 6, "no command: an address that does not parse is shown as it was typed"),
    ("audio.play", 1, "no command: its handler is the player's own"),
];

/// The command asked for in what it visits, the last one.
#[derive(Default)]
struct Asked(Option<String>);

impl<'a> Visit<'a> for Asked {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        walk::walk_call_expression(self, call);
        let invoked = match &call.callee {
            Expression::Identifier(name) => name.name == "invoke",
            Expression::StaticMemberExpression(member) => member.property.name == "invoke",
            _ => false,
        };
        if invoked && let Some(Argument::StringLiteral(name)) = call.arguments.first() {
            self.0 = name.value.as_str().map(str::to_string);
        }
    }
}

/// The first thing it visits calls: what a handler answers when no command is there to name it.
struct FirstCall<'s> {
    source: &'s str,
    found: Option<String>,
}

impl<'a> Visit<'a> for FirstCall<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        walk::walk_call_expression(self, call);
        if self.found.is_none() {
            self.found = Some(call.callee.span().source_text(self.source).to_string());
        }
    }

    fn visit_new_expression(&mut self, new: &NewExpression<'a>) {
        if self.found.is_none() {
            self.found = Some(new.callee.span().source_text(self.source).to_string());
        }
    }
}

/// Whether a handler uses a sentence: the page's words or what it caught, given to a call or
/// assigned — never only held in a declaration, a test or a value thrown away — or logs it.
#[derive(Default)]
struct Says {
    caught: Option<String>,
    using: bool,
    held: bool,
    says: bool,
}

impl Says {
    fn using<F: FnOnce(&mut Self)>(&mut self, visit: F) {
        let was = self.using;
        self.using = !self.held;
        visit(self);
        self.using = was;
    }
}

impl<'a> Visit<'a> for Says {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Identifier(name) = &call.callee
            && name.name == "pageFailed"
        {
            self.says = true;
        }
        self.visit_expression(&call.callee);
        self.using(|says| {
            for argument in &call.arguments {
                says.visit_argument(argument);
            }
        });
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        self.using(|says| says.visit_expression(&assignment.right));
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let was = self.held;
        self.held = true;
        walk::walk_variable_declarator(self, declarator);
        self.held = was;
    }

    fn visit_identifier_reference(&mut self, name: &IdentifierReference<'a>) {
        if self.using && (name.name == "words" || self.caught.as_deref() == Some(name.name.as_str())) {
            self.says = true;
        }
    }
}

/// Every handler of `page`.
struct Reading<'s> {
    source: &'s str,
    silent: BTreeMap<String, usize>,
}

impl Reading<'_> {
    fn found(&mut self, name: String) {
        *self.silent.entry(name).or_insert(0) += 1;
    }

    /// The answer of `receiver`, by the command in it, or else what it is.
    fn promise<'a>(&mut self, receiver: &Expression<'a>, handler: &Argument<'a>) {
        let mut asked = Asked::default();
        asked.visit_expression(receiver);
        let name = asked.0.unwrap_or_else(|| {
            let text = receiver.span().source_text(self.source);
            text.split('(').next().unwrap_or(text).to_string()
        });
        let says = match handler {
            Argument::ArrowFunctionExpression(arrow) => {
                let mut says = Says {
                    caught: first_parameter(&arrow.params),
                    ..Default::default()
                };
                says.visit_arrow_function_expression(arrow);
                says.says
            }
            Argument::FunctionExpression(function) => {
                let mut says = Says {
                    caught: first_parameter(&function.params),
                    ..Default::default()
                };
                if let Some(body) = &function.body {
                    says.visit_function_body(body);
                }
                says.says
            }
            Argument::Identifier(name) => name.name == "pageFailed",
            _ => false,
        };
        if !says {
            self.found(name);
        }
    }
}

fn first_parameter(params: &FormalParameters) -> Option<String> {
    params.items.first()?.pattern.get_identifier_name().map(|name| name.to_string())
}

impl<'a> Visit<'a> for Reading<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::StaticMemberExpression(member) = &call.callee {
            let handler = match member.property.name.as_str() {
                "catch" => call.arguments.first(),
                "then" => call.arguments.get(1),
                _ => None,
            };
            if let Some(handler) = handler {
                self.promise(&member.object, handler);
            }
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
        if let Some(handler) = &statement.handler {
            let caught = handler
                .param
                .as_ref()
                .and_then(|param| param.pattern.get_identifier_name())
                .map(|name| name.to_string());
            let mut says = Says {
                caught,
                ..Default::default()
            };
            says.visit_block_statement(&handler.body);
            if !says.says {
                let mut asked = Asked::default();
                asked.visit_block_statement(&statement.block);
                let name = asked.0.unwrap_or_else(|| {
                    let mut first = FirstCall {
                        source: self.source,
                        found: None,
                    };
                    first.visit_block_statement(&statement.block);
                    first.found.unwrap_or_default()
                });
                self.found(name);
            }
        }
        walk::walk_try_statement(self, statement);
    }
}

/// What the page handles without a word, and how many times.
fn silent(page: &str) -> BTreeMap<String, usize> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, page, SourceType::mjs()).parse();
    assert!(parsed.diagnostics.is_empty(), "the page does not parse: {:?}", parsed.diagnostics);
    let mut reading = Reading {
        source: page,
        silent: BTreeMap::new(),
    };
    reading.visit_program(&parsed.program);
    reading.silent
}

#[test]
fn a_command_that_fails_says_so_or_is_named_here_with_why() {
    let found = silent(include_str!("../ui/window.js"));
    let named: BTreeMap<String, usize> = SILENT.iter().map(|(name, count, _)| (name.to_string(), *count)).collect();
    assert_eq!(found, named, "a failure that says nothing: say it, or name it in SILENT with why");
}

#[test]
fn the_reading_tells_what_says_from_what_does_not() {
    let page = r#"
        await invoke("quiet", { x }).catch(() => []);
        await invoke("told").catch((why) => { said.textContent = fill(words.no, why); });
        try { await invoke("hushed", { y: {} }); } catch { }
        try { await invoke("spoken"); } catch (why) { said.textContent = words.no; }
        try { new URL(text); } catch {}
        await invoke("ignored").catch((why) => {});
        await invoke("drawn").catch((why) => { said.textContent = why; });
        await invoke("logged").catch((why) => pageFailed(why));
        invoke("then1").then(ok, () => {});
        invoke("then2").then(null, () => []);
        invoke("thenSaid").then(ok, (why) => { said.textContent = why; });
        invoke("thenOnly").then(ok);
        invoke("function").catch(function () {});
        invoke("named").catch(noop);
        try { await invoke("tight"); }catch {}
        try { await invoke("newline"); }
        catch {}
        try { await invoke("voided"); } catch (why) { void why; }
        invoke("commented").catch(() => { /* why not */ });
        invoke("held").catch(() => { const q = words.none; });
        invoke("held2").catch(() => { let q = words.none; });
        try { await invoke("bare"); } catch (why) { why; }
        try { await invoke("tested"); } catch (why) { if (why) {} }
        invoke ("spaced") . catch (() => {});
        invoke("returned").catch((why) => why);
        invoke("thrown").catch(() => words.none);
        invoke("near").catch(() => { said.textContent = whyNot + keywords.x; });
        const p = invoke("kept");
        p.then(ok, () => {});
        p.catch(noop);
        ask("mine").catch(noop);
        try { await helper(); } catch {}
        const quoted = /'/; invoke("regex").catch(() => {});
    "#;
    let expected: BTreeMap<String, usize> = [
        ("URL", 1),
        ("ask", 1),
        ("bare", 1),
        ("commented", 1),
        ("function", 1),
        ("held", 1),
        ("held2", 1),
        ("helper", 1),
        ("hushed", 1),
        ("ignored", 1),
        ("named", 1),
        ("near", 1),
        ("newline", 1),
        ("p", 2),
        ("quiet", 1),
        ("regex", 1),
        ("returned", 1),
        ("spaced", 1),
        ("tested", 1),
        ("then1", 1),
        ("then2", 1),
        ("thrown", 1),
        ("tight", 1),
        ("voided", 1),
    ]
    .map(|(name, count)| (name.to_string(), count))
    .into();
    assert_eq!(silent(page), expected);
}
