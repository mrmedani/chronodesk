# ChronoDesk - Roadmap de Correction

## Objectif

Cette feuille de route transforme l'audit technique ChronoDesk en plan de correction progressif. Elle vise a remettre le projet dans un etat stable, securise et maintenable sans introduire de regressions.

Aucune correction ne doit etre faite hors de l'ordre defini ici. Chaque phase doit etre validee avant de passer a la suivante.

## Principes d'execution

- Corriger d'abord les blocages de compilation et de lancement.
- Traiter les risques de securite critiques avant les optimisations.
- Eviter les refactorings massifs tant que le comportement actuel n'est pas stabilise.
- Ajouter ou renforcer les tests avant les changements a fort impact.
- Ne pas melanger correction fonctionnelle, refactoring et optimisation dans la meme tache.

## Phase 1 - Stabilisation du Projet

### Objectif

Obtenir une base qui compile, se lance, charge la DLL Rust depuis Flutter et permet de verifier les corrections suivantes.

### Taches

#### TASK-001 - Inclure et exporter le module FFI Rust

- Priorite : Critique
- Description : declarer correctement `src/ffi.rs` dans la lib Rust afin que les symboles attendus par Flutter soient exportes.
- Fichiers concernes :
  - `src/lib.rs`
  - `src/ffi.rs`
  - `chronodesk_flutter/lib/src/ffi/native.dart`
- Bugs lies : BUG-001
- Dependances : aucune
- Risques :
  - Peut reveler des erreurs Rust actuellement masquees.
  - Peut faire echouer `cargo check` tant que `ffi.rs` n'est pas compilable.
- Benefice attendu : Flutter pourra trouver `chronodesk_init`, `chronodesk_get_peer_id` et les autres fonctions natives.
- Difficulté : Facile
- Temps estime : 30 a 60 minutes

#### TASK-002 - Corriger les `switch` Dart non termines

- Priorite : Critique
- Description : ajouter des terminaisons explicites aux `case` Dart dans la gestion d'evenements UI.
- Fichier concerne :
  - `chronodesk_flutter/lib/src/screens/home_screen.dart`
- Bugs lies : BUG-002
- Dependances : aucune
- Risques :
  - La logique d'evenements peut changer si le fall-through etait suppose, meme s'il est invalide en Dart.
- Benefice attendu : `flutter analyze` et la compilation Flutter peuvent passer.
- Difficulté : Facile
- Temps estime : 30 minutes

#### TASK-003 - Corriger les erreurs Rust revelees dans `ffi.rs`

- Priorite : Critique
- Description : corriger les problemes de borrow/scope dans les transferts de fichiers et tout autre probleme rendu visible apres inclusion du module FFI.
- Fichier concerne :
  - `src/ffi.rs`
- Bugs lies : BUG-004, BUG-017, BUG-018
- Dependances :
  - TASK-001
- Risques :
  - Regression sur transfert fichier.
  - Modification delicate car `ffi.rs` orchestre reseau, media, fichiers et etat global.
- Benefice attendu : compilation Rust stable avec le module FFI reellement inclus.
- Difficulté : Moyenne
- Temps estime : 1 a 2 heures

#### TASK-004 - Corriger le script de build Windows

- Priorite : Haute
- Description : remplacer la cible `chronodesk_app` par l'interface active `chronodesk_flutter`.
- Fichier concerne :
  - `build/build_windows.bat`
- Bugs lies : BUG-005, BUG-024
- Dependances :
  - TASK-001
  - TASK-002
- Risques :
  - Peut necessiter d'ajouter la copie de la DLL Rust dans le bon dossier Flutter.
- Benefice attendu : le build Windows manuel produit l'application active.
- Difficulté : Facile
- Temps estime : 15 a 30 minutes

### Validation obligatoire Phase 1

Le projet ne passe pas a la phase suivante tant que tous les controles suivants ne sont pas verts :

- `cargo fmt --all --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets -- -D warnings`
- `flutter analyze`
- `flutter test`
- Build Windows manuel ou CI equivalent
- Lancement Flutter avec chargement effectif de la DLL Rust

## Phase 2 - Securite Critique

### Objectif

Supprimer les risques immediats de compromission et etablir une base de securite minimale.

### Taches

#### TASK-005 - Revoquer et supprimer la cle SSH exposee

- Priorite : Critique
- Description : revoquer la cle privee presente dans le depot, remplacer l'acces serveur, puis retirer la cle du projet et de l'historique si necessaire.
- Fichiers concernes :
  - `ssh server key oracl/ssh-key-2026-06-27.key`
  - `ssh server key oracl/ssh-key-2026-06-27.key.pub`
  - `ssh server key oracl.zip`
- Bugs lies : BUG-003
- Dependances : aucune
- Risques :
  - Perte d'acces serveur si la nouvelle cle n'est pas installee avant revocation.
  - Purge Git delicate si le depot est deja partage.
- Benefice attendu : suppression du risque de compromission serveur.
- Difficulté : Moyenne
- Temps estime : 1 a 3 heures

#### TASK-006 - Supprimer les credentials TURN faibles par defaut

- Priorite : Haute
- Description : remplacer les credentials hardcodes par une configuration obligatoire via variables d'environnement ou secret manager.
- Fichiers concernes :
  - `server/turn/turnserver.conf`
  - `server/deploy-turn.sh`
- Bugs lies : BUG-007
- Dependances :
  - TASK-005 recommande
- Risques :
  - Le deploiement TURN echoue si les variables ne sont pas fournies.
- Benefice attendu : evite l'abus public du relais TURN.
- Difficulté : Facile
- Temps estime : 1 a 2 heures

#### TASK-007 - Rendre la verification update obligatoire

- Priorite : Haute
- Description : refuser l'installation si le checksum ou la signature est absent ou invalide.
- Fichier concerne :
  - `chronodesk_flutter/lib/src/update_checker.dart`
- Bugs lies : BUG-008, BUG-009
- Dependances :
  - Phase 1 terminee
- Risques :
  - Les mises a jour seront bloquees tant que les releases ne publient pas les artefacts de verification.
- Benefice attendu : reduit fortement le risque supply-chain.
- Difficulté : Moyenne
- Temps estime : 0.5 a 1 jour

#### TASK-008 - Ajouter authentification et pairing du signaling

- Priorite : Haute
- Description : proteger l'enregistrement des peers et empecher l'usurpation d'identite par simple reuse de `peer_id`.
- Fichiers concernes :
  - `src/bin/signaling.rs`
  - `src/network/signaling.rs`
  - `src/network/transport.rs`
  - `src/ffi.rs`
  - `chronodesk_flutter/lib/src/screens/home_screen.dart`
- Bugs lies : BUG-006, BUG-022
- Dependances :
  - Phase 1 terminee
- Risques :
  - Rupture de compatibilite avec les anciens clients.
  - Flux UX de connexion a redefinir.
- Benefice attendu : protection contre usurpation, DoS simple et connexions non autorisees.
- Difficulté : Difficile
- Temps estime : 3 a 7 jours

### Validation obligatoire Phase 2

- Scan secrets manuel et automatise.
- Aucun mot de passe par defaut dans les fichiers de deploiement.
- Test TURN avec credentials non hardcodes.
- Update refusee si checksum/signature absent.
- Signaling refuse un peer non authentifie.
- Connexion legitime toujours possible.

## Phase 3 - Stabilisation Fonctionnelle Core

### Objectif

Stabiliser les fonctions principales : video, input, fichier, audio et clipboard.

### Taches

#### TASK-009 - Corriger le pipeline video

- Priorite : Haute
- Description : aligner resolution capture/encodeur, clarifier les codecs supportes et desactiver H.264 cote envoi tant que le viewer ne sait pas le decoder.
- Fichiers concernes :
  - `src/video.rs`
  - `src/ffi.rs`
- Bugs lies : BUG-010, BUG-011, BUG-019
- Dependances :
  - Phase 1 terminee
- Risques :
  - Regression affichage ecran distant.
  - Augmentation temporaire CPU si fallback WebP/JPEG uniquement.
- Benefice attendu : affichage distant fiable.
- Difficulté : Difficile
- Temps estime : 2 a 5 jours

#### TASK-010 - Corriger le mapping souris

- Priorite : Haute
- Description : convertir correctement les coordonnees du widget Flutter vers les coordonnees reelles de l'image distante.
- Fichier concerne :
  - `chronodesk_flutter/lib/src/screens/home_screen.dart`
- Bugs lies : BUG-012
- Dependances :
  - TASK-009 recommande
- Risques :
  - Regression sur zoom/pan `InteractiveViewer`.
- Benefice attendu : clics et mouvements souris coherents.
- Difficulté : Moyenne
- Temps estime : 1 jour

#### TASK-011 - Stabiliser audio et mute

- Priorite : Moyenne
- Description : rendre le bouton mute fonctionnel ou retirer l'action trompeuse, puis verifier le flux audio reel.
- Fichiers concernes :
  - `src/audio.rs`
  - `src/ffi.rs`
  - `chronodesk_flutter/lib/src/screens/home_screen.dart`
- Bugs lies : BUG-013, BUG-015, BUG-016
- Dependances :
  - Phase 1 terminee
- Risques :
  - Le support audio systeme peut varier selon OS.
- Benefice attendu : UX honnete et audio plus stable.
- Difficulté : Moyenne a Difficile
- Temps estime : 1 a 4 jours selon portee

#### TASK-012 - Securiser le transfert de fichiers

- Priorite : Moyenne
- Description : ne creer le fichier `.part` qu'apres acceptation, ajouter limite de taille, verification espace disque et erreurs explicites.
- Fichiers concernes :
  - `src/ffi.rs`
  - `src/file_transfer.rs`
  - `chronodesk_flutter/lib/src/screens/home_screen.dart`
- Bugs lies : BUG-017, BUG-018
- Dependances :
  - TASK-003
- Risques :
  - Regression sur progression et annulation.
- Benefice attendu : transfert fichier plus sur et plus previsible.
- Difficulté : Moyenne
- Temps estime : 1 a 2 jours

### Validation obligatoire Phase 3

- Deux instances peuvent se connecter.
- Demande entrante accept/deny fonctionne.
- Ecran distant visible.
- Input souris/clavier coherent.
- Transfert fichier accepte, refuse et annule.
- Deconnexion nettoie l'etat.
- Audio/mute verifie ou fonctionnalite masquee.

## Phase 4 - Performance et Ressources

### Objectif

Reduire la latence, le CPU, les allocations inutiles et les fuites de ressources.

### Taches

#### TASK-013 - Ajouter shutdown propre clipboard/audio

- Priorite : Moyenne
- Description : arreter les threads clipboard et audio a la deconnexion.
- Fichiers concernes :
  - `src/clipboard.rs`
  - `src/audio.rs`
  - `src/ffi.rs`
- Bugs lies : BUG-014
- Dependances :
  - Phase 3 terminee
- Risques :
  - Deadlock si les threads attendent mal les signaux d'arret.
- Benefice attendu : sessions longues plus stables.
- Difficulté : Moyenne
- Temps estime : 1 jour

#### TASK-014 - Reutiliser `InputController`

- Priorite : Moyenne
- Description : conserver un controleur input par session au lieu de le recreer a chaque evenement.
- Fichier concerne :
  - `src/ffi.rs`
- Bugs lies : BUG-021
- Dependances :
  - Phase 1 terminee
- Risques :
  - Etat input persistant a nettoyer sur deconnexion.
- Benefice attendu : latence input reduite.
- Difficulté : Facile
- Temps estime : 1 a 2 heures

#### TASK-015 - Rendre l'encodeur video persistant

- Priorite : Moyenne
- Description : eviter de recreer les contextes encodeur/scaler par frame et appliquer reellement la qualite adaptative.
- Fichiers concernes :
  - `src/video.rs`
  - `src/ffi.rs`
- Bugs lies : BUG-019, BUG-020
- Dependances :
  - TASK-009
- Risques :
  - Gestion complexe des changements de resolution.
- Benefice attendu : CPU et latence video reduits.
- Difficulté : Difficile
- Temps estime : 2 a 4 jours

### Validation obligatoire Phase 4

- Mesure CPU/RAM avant/apres.
- Session de 30 minutes sans croissance memoire anormale.
- Latence input acceptable.
- Aucun thread residuel apres deconnexion.

## Phase 5 - Architecture et Dette Technique

### Objectif

Reduire les risques futurs en clarifiant les responsabilites.

### Taches

#### TASK-016 - Decouper `src/ffi.rs`

- Priorite : Haute
- Description : separer configuration, events, connection, media, fichiers et etat applicatif.
- Fichiers concernes :
  - `src/ffi.rs`
  - nouveaux modules Rust a definir
- Bugs lies : BUG-025 et dette globale
- Dependances :
  - Phases 1 a 3 validees
- Risques :
  - Regression transversale majeure si fait trop tot.
- Benefice attendu : maintenabilite et testabilite fortement ameliorees.
- Difficulté : Difficile
- Temps estime : 5 a 10 jours

#### TASK-017 - Supprimer ou archiver `chronodesk_app`

- Priorite : Moyenne
- Description : retirer le prototype Flutter obsolete apres validation de `chronodesk_flutter`.
- Fichiers concernes :
  - `chronodesk_app/`
  - `build/build_windows.bat`
  - documentation
- Bugs lies : BUG-024
- Dependances :
  - TASK-004
- Risques :
  - Aucun si le dossier n'est plus utilise.
- Benefice attendu : moins de confusion build/release.
- Difficulté : Facile
- Temps estime : 30 minutes

#### TASK-018 - Nettoyer les dependances inutilisees

- Priorite : Faible
- Description : supprimer ou justifier les dependances non utilisees.
- Fichiers concernes :
  - `Cargo.toml`
  - `chronodesk_flutter/pubspec.yaml`
- Bugs lies : BUG-023
- Dependances :
  - Tests fiables disponibles
- Risques :
  - Retrait d'une dependance utilisee indirectement.
- Benefice attendu : surface de maintenance reduite.
- Difficulté : Facile
- Temps estime : 1 heure

### Validation obligatoire Phase 5

- Meme checklist que Phase 1.
- Tests fonctionnels Phase 3 rejoues.
- Aucun changement comportemental non documente.

## Phase 6 - Tests et Documentation

### Objectif

Installer un filet de securite durable contre les regressions.

### Taches

#### TASK-019 - Ajouter tests unitaires Rust

- Priorite : Haute
- Description : couvrir `file_transfer`, `input`, `video`, `protocol` et cas limites.
- Fichiers concernes :
  - `src/file_transfer.rs`
  - `src/input.rs`
  - `src/video.rs`
  - `src/protocol.rs`
- Bugs lies : BUG-025
- Dependances :
  - Phases 1 a 3 stabilisees
- Risques :
  - Tests fragiles si l'architecture n'est pas encore clarifiee.
- Benefice attendu : regressions detectees plus tot.
- Difficulté : Moyenne
- Temps estime : 2 a 4 jours

#### TASK-020 - Ajouter tests integration signaling/transport

- Priorite : Haute
- Description : simuler deux peers et verifier register, offer, answer, ICE et data channel.
- Fichiers concernes :
  - `src/bin/signaling.rs`
  - `src/network/signaling.rs`
  - `src/network/transport.rs`
- Bugs lies : BUG-006, BUG-025
- Dependances :
  - TASK-008
- Risques :
  - Tests reseau potentiellement flaky si ports et timeouts mal controles.
- Benefice attendu : confiance sur le coeur P2P.
- Difficulté : Difficile
- Temps estime : 3 a 5 jours

#### TASK-021 - Ajouter tests Flutter widget

- Priorite : Moyenne
- Description : tester dialogs, gestion evenements, progress transferts et etats de connexion.
- Fichier concerne :
  - `chronodesk_flutter/test/`
- Bugs lies : BUG-002, BUG-013, BUG-025
- Dependances :
  - Phase 1 terminee
- Risques :
  - Necessite mock FFI pour eviter dependance a la DLL native.
- Benefice attendu : UI moins fragile.
- Difficulté : Moyenne
- Temps estime : 2 a 3 jours

#### TASK-022 - Mettre a jour la documentation technique

- Priorite : Moyenne
- Description : documenter architecture active, securite, update, release et procedure de validation.
- Fichiers concernes :
  - `README.md`
  - `SECURITY.md`
  - `CONTRIBUTING.md`
  - `docs/`
- Bugs lies : documentation incomplete ou optimiste
- Dependances :
  - Phases principales implementees
- Risques :
  - Documentation obsolete si ecrite avant stabilisation.
- Benefice attendu : onboarding et maintenance facilites.
- Difficulté : Facile
- Temps estime : 1 a 2 jours

### Validation obligatoire Phase 6

- CI complete verte.
- Tests unitaires, integration et Flutter verts.
- Checklist manuelle release complete.
- Documentation coherente avec le code.

## Ordre optimal d'execution

1. TASK-005 - Revoquer la cle SSH exposee.
2. TASK-002 - Corriger compilation Flutter.
3. TASK-001 - Inclure FFI Rust.
4. TASK-003 - Corriger erreurs Rust revelees.
5. TASK-004 - Corriger build Windows.
6. Valider Phase 1.
7. TASK-006 - Corriger TURN credentials.
8. TASK-007 - Securiser auto-update.
9. TASK-008 - Ajouter auth/pairing signaling.
10. Valider Phase 2.
11. TASK-009 - Stabiliser video.
12. TASK-010 - Corriger input souris.
13. TASK-012 - Securiser fichiers.
14. TASK-011 - Stabiliser audio/mute.
15. Valider Phase 3.
16. TASK-013, TASK-014, TASK-015 - Performance.
17. TASK-016, TASK-017, TASK-018 - Architecture et dette.
18. TASK-019, TASK-020, TASK-021, TASK-022 - Tests et docs.

## Definition de fini globale

Le projet est considere stabilise lorsque :

- La CI compile Rust et Flutter.
- L'application active est `chronodesk_flutter`.
- La DLL Rust exporte toutes les fonctions FFI attendues.
- Aucun secret prive n'est present dans le depot.
- Le signaling possede un mecanisme d'authentification/pairing.
- Le pipeline video/input fonctionne de bout en bout.
- Les transferts de fichiers sont limites, consentis et testables.
- Les tests couvrent crypto, protocol, fichiers, signaling et UI principale.
- La documentation de build, release et securite est a jour.
