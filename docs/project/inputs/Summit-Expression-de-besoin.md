# Summit — Expression de besoin

Oct 12, 2026 · @fc

## 1. Vision et problème

Summit est une application de bureau qui enregistre une conversation, la transcrit et en tire un compte rendu, sans jamais imposer ni un fournisseur d'IA ni un mode de fonctionnement. Elle tourne sur Windows, macOS et Linux et se fait oublier : elle vit dans la barre système et dans le menu contextuel de l'OS, pas dans une fenêtre de plus.

**Le problème.** Les note takers actuels obligent à choisir un camp. Soit un service cloud complet et pratique, mais fermé : abonnement, données envoyées chez un tiers, modèle d'IA imposé, souvent un bot qui rejoint la réunion. Soit un outil local respectueux de la vie privée, mais limité à une plateforme ou à une seule brique (la transcription, sans résumé, ou l'inverse).

**La promesse.** Un flux simple en quatre étapes (capter, transcrire, résumer, exporter) où chaque étape est indépendante et se configure à part. On peut transcrire sur sa machine et résumer via OpenRouter, tout faire en local, ou n'utiliser qu'une seule étape (transcrire un fichier audio existant, résumer un texte collé).

**Pour qui.** D'abord des utilisateurs avancés et des petites équipes qui veulent garder la main sur leurs données et leurs coûts. Le produit doit rester utilisable par quelqu'un de non technique une fois configuré.

**Ce que Summit n'est pas.** Ni une plateforme de collaboration, ni un CRM, ni un outil d'analyse commerciale des appels.

## 2. Principes produit

Ces six principes tranchent les arbitrages : une fonctionnalité qui en viole un n'entre pas dans le produit.

1. **Minimaliste.** Peu d'écrans, peu de réglages visibles, des valeurs par défaut qui marchent. Les options avancées existent mais restent repliées.
2. **Modulaire.** Capture, transcription, résumé et export sont quatre étapes indépendantes. Chacune a son propre fournisseur, ses propres réglages, et peut être lancée seule.
3. **Le fournisseur appartient à l'utilisateur.** Moteur local ou service compatible OpenAI (OpenRouter, ou toute adresse personnalisée). Aucun fournisseur imposé, aucune clé fournie par Summit.
4. **Local d'abord.** Les fichiers restent sur la machine par défaut. Rien ne sort sans que l'utilisateur l'ait choisi, et l'interface dit toujours ce qui sort et vers où.
5. **Fondu dans l'OS.** Pas de fenêtre à garder ouverte : barre système, menu contextuel des fichiers, raccourci clavier global, notifications natives.
6. **Pas de bot, pas de compte.** Summit capte l'audio de la machine. Il ne rejoint jamais une réunion comme participant et ne demande aucune inscription.

## 3. Analyse du marché

Aucun produit existant ne combine les trois : multiplateforme Linux compris, chaque étape configurable à part, et intégration au menu contextuel. C'est l'espace de Summit.

| Produit | Approche | Ce qu'il fait bien | Faiblesses |
| --- | --- | --- | --- |
| [Otter](https://www.npr.org/2025/08/15/g-s1-83087/otter-ai-transcription-class-action-lawsuit) / [Fireflies](https://unstar.app/blog/otter-fireflies-notta-rev-tactiq-ai-meeting-transcription-apps-ranked-2026) | Cloud, un bot rejoint la réunion | Transcription en direct, intégrations CRM, peut assister à une réunion sans l'utilisateur | Bot visible et intrusif, consentement flou (action collective contre Otter en 2025), données chez un tiers, abonnement |
| [Granola](https://www.happyscribe.com/blog/granola-ai-review) | App de bureau sans bot, résumé guidé par les notes de l'utilisateur | Interface très épurée, le résumé reprend ce que l'utilisateur a noté, modèles de compte rendu | Traitement dans le cloud, compte obligatoire avec accès au calendrier, distinction des voix faible, pas de Linux, modèle d'IA imposé |
| [Jamie](https://www.meetjamie.ai/en/blog/granola-vs-bluedot-vs-jamie) | App sans bot, Mac / Windows / iOS | Fonctionne hors ligne et en présentiel, reconnaît les voix récurrentes | Offre gratuite plafonnée, fournisseur d'IA choisi par l'éditeur, pas de Linux |
| [Meetily](https://github.com/zackriya-meetily/meetily) | Open source, tout en local | Transcription et stockage locaux, résumé local ou via clé (OpenRouter, OpenAI…) | Mac et Windows seulement, centré réunion uniquement, pas d'échange avec les notes |
| [Anarlog](https://anarlog.so/blog/markdown-note-taking-apps) (ex-Hyprnote) | Open source, bloc-notes local | Choix libre du moteur de transcription et du modèle de résumé, export Markdown, fonctionne avec Ollama ou LM Studio | Pensé comme un éditeur de notes plus que comme un outil discret, Linux longtemps annoncé comme incertain |
| [MacWhisper](https://goodsnooze.gumroad.com/l/macwhisper/bff2025p) | Transcription locale de fichiers | Glisser-déposer, dossiers surveillés, traitement par lot, appel rapide depuis n'importe où | Mac uniquement, pas de compte rendu structuré natif, fonctions avancées payantes |

**Ce que Summit reprend**

- Pas de bot : capter l'audio de la machine (Granola, Jamie, Meetily).
- La sobriété de Granola : un écran de résultat clair, pas un tableau de bord.
- Le choix libre des modèles de Meetily et Anarlog, mais étape par étape.
- Les fichiers ouverts d'Anarlog : un compte rendu lisible hors de l'application.
- Le traitement de fichiers de MacWhisper : glisser, dossier surveillé, lot.

**Ce que Summit évite**

- Compte, inscription ou accès au calendrier exigés pour démarrer.
- Fournisseur d'IA imposé ou abonnement obligatoire.
- Enregistrement silencieux : l'utilisateur est toujours averti et rappelle le consentement.
- Dépendance au cloud pour une fonction de base.
- Oubli de Linux.

## 4. Utilisateurs et cas d'usage

Summit sert trois profils, et le même outil doit couvrir les six situations ci-dessous sans mode spécial.

**Profils**

- **L'utilisateur soucieux de ses données** (juriste, consultant, santé, freelance) : veut tout en local, accepte un résultat un peu moins bon.
- **L'utilisateur pragmatique** : transcrit en local, résume via OpenRouter pour la qualité, surveille le coût.
- **Le bricoleur** : utilise une seule étape (transcrire un fichier, résumer un texte) et branche Summit dans ses propres habitudes.

**Cas d'usage**

| Situation | Étapes utilisées | Exemple |
| --- | --- | --- |
| Réunion en visio | Capture, transcription, résumé, export | Appel Meet ou Teams, compte rendu envoyé dans le dossier de notes |
| Réunion en présentiel | Capture micro seul, transcription, résumé | Point d'équipe autour d'un ordinateur |
| Fichier audio ou vidéo existant | Transcription, résumé | Clic droit sur un enregistrement de téléphone, « Transcrire avec Summit » |
| Texte ou transcription déjà disponible | Résumé seul | Clic droit sur un fichier texte, « Résumer avec Summit » |
| Mémo vocal rapide | Capture, transcription | Raccourci global, on parle, le texte est copié |
| Reprise d'une session | Résumé ou export relancé | Régénérer le résumé avec un autre modèle, renvoyer vers un autre outil |

## 5. Parcours utilisateur

Le parcours type d'une réunion tient en sept moments, dont un seul demande une action de l'utilisateur : accepter l'enregistrement.

1. **Premier lancement.** Un assistant de trois écrans maximum : choisir un profil prêt à l'emploi (« Tout en local », « Local + OpenRouter », « Personnalisé »), autoriser micro et son système, choisir le dossier des sessions. Summit est utilisable dès la fin de l'assistant.
2. **Au repos.** Summit vit dans la barre système. Son menu donne : démarrer un enregistrement, les dernières sessions, le coût du mois, les réglages.
3. **Le micro s'active** (n'importe quelle application). Selon le réglage : demander, enregistrer automatiquement, ou ne rien faire. La notification rappelle d'informer les participants.
4. **Pendant l'enregistrement.** Un indicateur discret et toujours visible (durée, état « Enregistrement »). La transcription en direct est disponible mais optionnelle. Pause, arrêt, abandon.
5. **Fin de la réunion.** L'enregistrement s'arrête seul quand le micro se coupe, ou à la demande. L'audio et la transcription sont déjà sauvegardés.
6. **Traitement.** Les étapes suivantes s'enchaînent selon la configuration. Une notification annonce le résultat, avec le coût s'il est connu.
7. **Consultation.** Une fenêtre unique montre le compte rendu, la transcription et l'audio. Depuis là : copier, exporter, relancer une étape.

Hors réunion, le parcours est encore plus court : clic droit sur un fichier, choix de l'action, notification à la fin.

## 6. Les quatre étapes du flux

Le flux se découpe en quatre étapes indépendantes : chacune a son entrée, sa sortie, son fournisseur et ses réglages, et on peut entrer dans le flux ou en sortir à n'importe quelle étape, en important ou en exportant le résultat intermédiaire.

&#91;embedded content: flux Summit · 4 étapes, 3 points d'entrée\]

Une réunion parcourt les quatre étapes ; un audio importé commence à la transcription, une transcription importée directement au résumé. Chaque résultat intermédiaire peut aussi être exporté.

| Étape | Entrée | Sortie | Fournisseurs possibles | Réglages propres |
| --- | --- | --- | --- | --- |
| 1. Capture | Micro et/ou son du système | Fichier audio | La machine | Sources captées, déclenchement (demander / auto / rien), arrêt automatique, durée de conservation de l'audio |
| 2. Transcription | Fichier audio ou vidéo | Texte horodaté, avec « Moi » et « Autres » | Moteur local, ou service en ligne compatible OpenAI | Moteur, langue (ou détection automatique), vocabulaire personnalisé, transcription en direct ou non |
| 3. Résumé | Texte (transcription ou autre) | Compte rendu structuré + titre | Modèle local, OpenRouter, ou toute adresse compatible OpenAI | Adresse, clé (facultative), modèle, modèle de compte rendu, langue de sortie |
| 4. Export | Compte rendu, transcription, audio | Fichiers ou envoi vers un outil | Dossier local, presse-papiers, outils tiers | Destinations, contenu envoyé, automatique ou manuel |

**Import et export entre les étapes**

Chaque résultat intermédiaire s'importe pour démarrer l'étape suivante et s'exporte pour être utilisé ailleurs ou retravaillé, puis réimporté.

| Étape | On peut importer | On peut exporter |
| --- | --- | --- |
| 1. Capture | Rien : l'import d'un audio remplace cette étape | L'audio brut (une piste par source ou les deux mélangées) |
| 2. Transcription | Un fichier audio ou vidéo, à la place du micro et du son système | La transcription, horodatée, avec les locuteurs, en texte brut ou sous-titres |
| 3. Résumé | Une transcription (de Summit, d'un autre outil, ou corrigée à la main) ou tout texte | Le compte rendu, en texte mis en forme ou en données structurées |
| 4. Export | Un compte rendu existant, pour l'envoyer vers un outil | Session complète (audio, transcription, compte rendu) |

- Une transcription exportée, corrigée dans un éditeur puis réimportée, sert de base au résumé à la place de l'originale.
- Les formats exportés par Summit sont toujours réimportables par Summit, sans perte (horodatage et locuteurs conservés).
- Les formats courants d'autres outils sont acceptés en import : texte brut, sous-titres, Markdown.
- Une session peut être exportée entière et réimportée sur une autre machine.

**Règles communes à toutes les étapes**

- Chaque étape peut être activée, désactivée ou mise en mode manuel. Exemple : capture et transcription automatiques, résumé seulement sur demande.
- Le résultat de chaque étape est conservé. Si une étape échoue, les précédentes ne sont pas perdues et l'étape peut être relancée.
- Chaque étape peut être relancée avec un autre fournisseur, sans refaire les autres.
- Un fournisseur se configure une fois et se réutilise : la même adresse OpenRouter peut servir à la transcription et au résumé.
- L'interface indique pour chaque étape si elle reste sur la machine ou sort vers un service en ligne.

* Toute étape démarre aussi bien depuis la sortie de l'étape précédente que depuis un fichier importé.

## 7. Besoins fonctionnels

Les besoins sont regroupés par domaine et numérotés pour pouvoir être cités dans la suite du projet.

**Capture**

- C1. Détecter que le micro s'active et proposer d'enregistrer, selon le réglage choisi.
- C2. Enregistrer le micro et le son du système comme deux sources distinctes (« Moi » et « Autres »).
- C3. Démarrer manuellement depuis la barre système ou un raccourci global.
- C4. Mettre en pause, reprendre, arrêter, abandonner.
- C5. Arrêter automatiquement quand le micro se coupe.
- C6. Afficher en permanence qu'un enregistrement est en cours.
- C7. Rappeler d'informer les participants, à chaque démarrage (désactivable).

**Transcription**

- T1. Transcrire en direct pendant l'enregistrement, ou après coup, au choix.
- T2. Transcrire n'importe quel fichier audio ou vidéo courant.
- T3. Gérer au moins le français et l'anglais, avec détection automatique de la langue.
- T4. Horodater chaque phrase et indiquer qui parle (« Moi » / « Autres »), noms renommables.
- T5. Ajouter un vocabulaire personnalisé (noms propres, jargon).

**Résumé**

- R1. Produire un compte rendu dans un ordre fixe : points forts, actions (avec responsable et échéance si mentionnés), points clés.
- R2. Relier chaque point clé à son moment dans l'audio.
- R3. Générer un titre de session, modifiable.
- R4. Proposer des modèles de compte rendu (réunion, entretien, cours, appel client) et permettre d'en créer.
- R5. Écrire le compte rendu dans la langue de la conversation, ou dans une langue imposée.
- R6. Régénérer le résumé avec un autre modèle ou un autre modèle de compte rendu.
- R7. Gérer les très longues conversations sans échec silencieux.

**Export**

- E1. Enregistrer chaque session comme des fichiers ouverts et lisibles sans Summit (texte, audio).
- E2. Copier le compte rendu en un clic, mis en forme ou en texte brut.
- E3. Exporter vers des outils tiers (Notion en premier, d'autres ensuite).
- E4. Envoyer automatiquement ou manuellement, au choix, avec un statut clair (envoyé, non envoyé, échec et raison).

**Import et export entre les étapes**

- I1. Importer un fichier audio ou vidéo à la place de la capture.
- I2. Exporter l'audio d'une session.
- I3. Exporter la transcription et importer une transcription (de Summit ou d'un autre outil) pour lancer le résumé.
- I4. Exporter le compte rendu et importer un compte rendu existant pour l'envoyer vers un outil.
- I5. Remplacer le résultat d'une étape par une version corrigée importée ; les étapes suivantes repartent de cette version.
- I6. Exporter une session entière et la réimporter, sur la même machine ou une autre.

**Sessions et recherche**

- S1. Lister les sessions : date, titre, durée, application source, coût, statut d'export.
- S2. Chercher dans les titres, comptes rendus et transcriptions.
- S3. Lire l'audio, avec saut au moment d'une phrase ou d'un point clé.
- S4. Supprimer une session et tout ce qu'elle contient.

**Coûts**

- K1. Afficher le coût de chaque étape quand le fournisseur le communique, sinon l'indiquer (« non communiqué » ou « gratuit, local »).
- K2. Afficher le total du mois.
- K3. Avertir quand un seuil mensuel est dépassé.

**Réglages**

- P1. Proposer des profils prêts à l'emploi et un mode personnalisé.
- P2. Configurer chaque étape séparément, avec un bouton « Tester » par fournisseur.
- P3. Lister automatiquement les modèles disponibles chez un fournisseur quand c'est possible, sinon saisie libre.
- P4. Importer et exporter la configuration.

## 8. Intégration à l'OS

Summit doit se comporter comme un outil du système, pas comme une application qu'on ouvre : quatre points d'entrée, tous disponibles sur Windows, macOS et Linux.

| Point d'entrée | Ce qu'il offre |
| --- | --- |
| Barre système (zone de notification Windows, barre des menus macOS, zone de notification Linux) | Démarrer / arrêter, état en cours, dernières sessions, coût du mois, réglages |
| Menu contextuel des fichiers (clic droit dans l'explorateur, le Finder, le gestionnaire de fichiers) | Sur un audio ou une vidéo : « Transcrire », « Transcrire et résumer ». Sur un texte : « Résumer ». Sur une session : « Exporter » |
| Raccourci clavier global | Démarrer / arrêter l'enregistrement, mémo vocal rapide copié dans le presse-papiers |
| Notifications natives | Proposition d'enregistrer, fin de traitement, échec avec action possible (« Réessayer ») |

**Exigences d'intégration**

- Apparence native sur chaque OS : thème clair et sombre suivis, polices et contrôles du système.
- Les actions du menu contextuel tournent en arrière-plan et répondent par une notification, sans ouvrir de fenêtre.
- Un dossier surveillé, facultatif : tout fichier audio déposé est traité selon la configuration.
- Démarrage avec la session de l'OS, désactivable.
- Les mêmes fonctions sur les trois OS. Si une fonction n'est pas possible sur l'un d'eux, elle est signalée dans les réglages plutôt que cachée.

## 9. Exigences non fonctionnelles

**Confidentialité**

- Par défaut, rien ne quitte la machine. Toute sortie de données est un choix explicite de l'utilisateur.
- Seul le texte part vers un service de résumé en ligne ; l'audio ne part que vers une transcription en ligne ou un export choisis.
- Clés et jetons d'accès stockés dans le coffre sécurisé de l'OS, jamais en clair.
- Aucune télémétrie sans accord explicite.

**Simplicité**

- Un premier compte rendu obtenu en moins de 5 minutes après l'installation, avec un profil prêt à l'emploi.
- Un utilisateur non technique comprend chaque réglage visible sans documentation.
- Une seule fenêtre principale ; tout le reste passe par la barre système, le menu contextuel et les notifications.

**Fiabilité**

- Aucun enregistrement perdu : en cas de plantage, l'audio capté jusque-là est récupéré au redémarrage.
- Une réunion de 2 heures se traite de bout en bout.
- Les erreurs disent ce qui s'est passé et quoi faire, en une phrase.

**Discrétion**

- Pendant une réunion, Summit ne ralentit pas sensiblement la visio ni la machine.
- Aucune fenêtre ne s'ouvre sans action de l'utilisateur.

**Coûts**

- Summit lui-même ne facture rien à l'usage. Les seuls coûts sont ceux des fournisseurs choisis par l'utilisateur, affichés tels que communiqués, jamais estimés à tort.

**Ouverture**

- Les sessions restent lisibles si Summit est désinstallé.
- Ajouter un nouveau fournisseur compatible OpenAI ne demande qu'une adresse, une clé et un nom de modèle.

## 10. Périmètre

Le MVP couvre le flux complet sur les trois OS avec un seul outil d'export (Notion) ; tout ce qui enrichit sans être indispensable vient ensuite.

| Domaine | MVP | Plus tard |
| --- | --- | --- |
| Capture | C1 à C7 | Règles par application, arrêt sur silence |
| Transcription | T1 à T4, moteur local + service compatible OpenAI | T5 vocabulaire, distinction des voix au-delà de Moi / Autres |
| Résumé | R1 à R3, R5 à R7, un modèle de compte rendu par défaut modifiable | R4 bibliothèque de modèles |
| Export | E1, E2, Notion pour E3, E4 | Obsidian, Google Docs, e-mail, webhook |
| Import / export entre étapes | I1 à I5 | I6 session entière |
| Sessions | S1 à S4 | Actions groupées au-delà de l'export |
| Coûts | K1, K2 | K3 alerte de seuil |
| Réglages | P1 à P3 | P4 import / export de la configuration |
| Intégration OS | Barre système, menu contextuel, raccourci global, notifications | Dossier surveillé |

**Hors périmètre**

- Bot qui rejoint une réunion.
- Compte utilisateur, synchronisation cloud, collaboration en équipe.
- Application mobile.
- Discussion avec l'ensemble de ses réunions (« pose une question à tes notes »).
- Analyse commerciale des appels, intégration CRM.
- Tout ce qui sert à vendre le produit : licence, paiement, boutique.

## 11. Critères d'acceptation et questions ouvertes

**Critères d'acceptation du MVP**

- [ ] Sur Windows, macOS et Linux, une visio Meet ou Teams en français déclenche la proposition d'enregistrer en quelques secondes.
- [ ] Pendant l'appel, des lignes apparaissent pour « Moi » et pour « Autres ».
- [ ] À la fin de l'appel, l'enregistrement s'arrête seul et le compte rendu arrive avec points forts, actions et points clés ; chaque point clé ramène au bon moment de l'audio.
- [ ] Configuration mixte : transcription locale + résumé via OpenRouter fonctionne, et le coût affiché est celui communiqué par OpenRouter.
- [ ] Configuration 100 % locale : un compte rendu est produit sans connexion internet et sans clé, avec la mention « gratuit, local ».
- [ ] Clic droit sur un fichier audio, « Transcrire et résumer » : le résultat arrive par notification, sans ouvrir de fenêtre.
- [ ] Clic droit sur un fichier texte, « Résumer » : seule l'étape résumé est exécutée. Une transcription exportée, corrigée à la main puis réimportée, produit un nouveau résumé fondé sur la version corrigée.
- [ ] Un échec du résumé ne fait perdre ni l'audio ni la transcription, et le résumé peut être relancé.
- [ ] Avec export automatique, une page Notion est créée ; si l'audio dépasse la limite de l'espace Notion, la page est créée sans audio et la raison est affichée.
- [ ] Quitter et relancer conserve toutes les sessions.

**Questions ouvertes (produit)**

- Le nom « Summit » est-il disponible (marque, domaine) ?
- Par défaut à la détection du micro : demander, ou ne rien faire tant que l'utilisateur n'a pas choisi ?
- La transcription en direct est-elle affichée par défaut, ou repliée pour rester discret ?
- Faut-il un espace de notes personnelles pendant la réunion, que le résumé prend en compte (approche Granola) ?
- Quelle durée de conservation de l'audio par défaut : illimitée, 30 jours, ou suppression après transcription ?
- Après Notion, quel export en priorité : Obsidian (fichiers Markdown), Google Docs ou e-mail ?
- Le produit sera-t-il open source ? Le choix change la confiance des profils soucieux de leurs données.
- Quel coût afficher pour un fournisseur qui ne le communique pas : rien, nombre de mots traités, ou prix saisi par l'utilisateur ?

**Sources**

- Brief Shorthand (pièce jointe), 2026-10-11.
- [Granola, revue HappyScribe](https://www.happyscribe.com/blog/granola-ai-review) · [Granola, revue SkillScouter](https://skillscouter.com/granola-review/)
- [Action collective contre Otter, NPR](https://www.npr.org/2025/08/15/g-s1-83087/otter-ai-transcription-class-action-lawsuit) · [Plaintes sur Otter, Fireflies, Notta, Unstar](https://unstar.app/blog/otter-fireflies-notta-rev-tactiq-ai-meeting-transcription-apps-ranked-2026)
- [Meetily, GitHub](https://github.com/zackriya-meetily/meetily) · [Anarlog (ex-Hyprnote)](https://anarlog.so/blog/markdown-note-taking-apps)
- [Jamie, comparatif Granola / Bluedot / Jamie](https://www.meetjamie.ai/en/blog/granola-vs-bluedot-vs-jamie) · [MacWhisper](https://goodsnooze.gumroad.com/l/macwhisper/bff2025p)
