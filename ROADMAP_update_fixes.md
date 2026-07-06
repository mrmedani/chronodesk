# ROADMAP — Correctifs du module de mise à jour

## Problème rapporté
> Le bouton Update télécharge et lance l'installateur, mais au prochain lancement l'application est toujours à l'ancienne version.

## Analyse approfondie

### Flux complet de la mise à jour

1. `checkForUpdate()` → API GitHub → compare semver
2. `downloadAndApplyUpdate()` → télécharge `chronodesk-windows-setup.exe`
3. `_verifyChecksum()` → SHA256 avec `certutil`
4. `_runInstaller()` → PowerShell élevé → attend la fermeture → installe → `exit(0)`

---

## BUGS IDENTIFIÉS

### BUG-UPDATE-01 — CRITICAL — Installateur installe au mauvais endroit

**Fichier**: `installer.iss:13`
```
DefaultDirName={autopf}\{#MyAppName}  →  C:\Program Files\Chronodesk
```

**Problème**: L'utilisateur lance l'app depuis le dossier de build ou un chemin personnalisé, mais l'installateur Inno Setup installe toujours dans `C:\Program Files\Chronodesk`. La mise à jour remplace les fichiers du Program Files, mais pas ceux que l'utilisateur lance réellement.

**Solution**: Détecter le chemin de l'exécutable actuel (`Platform.resolvedExecutable`) et soit :
- Passer le chemin en paramètre à l'installateur (`/DIR="C:\current\path"`)
- OU copier le nouvel exe directement dans le dossier parent de l'exécutable courant via PowerShell avant de lancer l'installateur système

---

### BUG-UPDATE-02 — HAUTE — `exit(0)` peut tuer le processus PowerShell avant la fin

**Fichier**: `update_checker.dart:238-239`
```dart
await Future.delayed(const Duration(seconds: 3));
exit(0);
```

**Problème**:
- `exit(0)` force la fin du processus Dart immédiatement après 3 secondes
- Même avec `ProcessStartMode.detached`, sous Windows le processus PowerShell peut être attaché à un **Job Object** qui est tué quand le parent meurt
- Si le `Start-Process` élévé n'a pas encore démarré (latence UAC), l'installateur ne s'exécute jamais

**Solution**: 
- Utiliser un événement de synchronisation (Mutex/Event) au lieu d'un délai fixe
- Ou ne pas appeler `exit(0)` du tout — laisser le PowerShell tuer le processus lui-même
- Ou utiliser `Process.start` avec `CREATE_BREAKAWAY_FROM_JOB` (via FFI)

---

### BUG-UPDATE-03 — HAUTE — `Get-Process` ne trouve pas le bon processus

**Fichier**: `update_checker.dart:209-211`
```dart
final exeName = Platform.resolvedExecutable
    .split('\\')
    .last
    .replaceAll('.exe', '');
```

**Problèmes**:
- Si l'exécutable est renommé par l'utilisateur, `$exeName` ne correspond plus
- `Platform.resolvedExecutable` peut retourner un chemin différent selon comment l'app est lancée (raccourci vs direct)
- Sur Windows, `Get-Process -Name` sans `.exe` peut être ambigu si le nom est tronqué

**Solution**: 
- Utiliser `Get-Process -Id $pid` depuis PowerShell (transmettre le PID réel au script)
- Au lieu de chercher par nom, le script reçoit le PID de l'app et attend que ce PID spécifique se termine

---

### BUG-UPDATE-04 — MOYENNE — Parsing du checksum cassé sur Windows non-anglais

**Fichier**: `update_checker.dart:178`
```dart
final actualHash = output.split('\n').skip(1).first.trim().replaceAll(' ', '');
```

**Problème**: `certutil -hashfile` sur Windows FR retourne :
```
Hachage SHA256 de <fichier> :
<hash>
```
Le `split('\n').skip(1)` fonctionne mais la ligne 2 peut contenir des espaces insécables ou un format différent. Sur Windows JA/CN/AR, le format est encore différent.

**Solution**: Utiliser une regex pour extraire le hash : `RegExp(r'^[a-f0-9]{64}$', multiLine: true)` au lieu de `skip(1).first`.

---

### BUG-UPDATE-05 — MOYENNE — Pas de vérification que l'installateur a réussi

**Fichier**: `_runInstaller()` ne retourne pas le code de sortie de l'installateur

**Problème**: `Start-Process -Wait` attend la fin, mais si l'installateur échoue (code != 0), personne n'est notifié. L'utilisateur voit la boîte de dialogue se fermer et croit que la mise à jour a réussi.

**Solution**: Vérifier `$LASTEXITCODE` après l'installateur et écrire un fichier de log ou afficher une erreur.

---

### BUG-UPDATE-06 — MOYENNE — Nettoyage de fichier partiel pendant l'annulation

**Fichier**: `update_checker.dart:126-133`

**Problème**: Quand `cancelUpdate()` ferme le client HTTP, la stream erreur déclenche le `catch`, qui supprime le fichier partiel et `rethrow`. Mais si l'annulation arrive EXACTEMENT entre la fin du stream et la vérification du checksum, le fichier partiel n'est pas nettoyé.

**Solution**: Enregistrer le chemin du fichier et le nettoyer dans un `finally` garanti.

---

### BUG-UPDATE-07 — FAIBLE — Comparaison de versions pré-release imprécise

**Fichier**: `update_checker.dart:78-86`

**Problème**: `_parseSegments` s'arrête au premier segment non-numérique. `"0.4.3-alpha"` et `"0.4.3-beta"` sont tous deux traités comme `"0.4.3"`. La comparaison pré-release se fait par `aParts[1].compareTo(bParts[1])` qui compare des strings, pas des semver structurés.

**Solution**: Utiliser le package `semver` ou implémenter le parsing complet selon la spec SemVer 2.0.

---

### BUG-UPDATE-08 — FAIBLE — Pas de fallback HTTP pour le checksum

**Fichier**: `update_checker.dart:149-150`

**Problème**: L'URL du checksum est en HTTPS uniquement. Si le certificat GitHub est expiré ou si le proxy d'entreprise bloque, la vérification échoue et l'installateur est supprimé.

**Solution**: Ajouter un fallback HTTP ou une option `--no-checksum` pour les environnements restreints.

---

## Plan de correction

## État des correctifs (v0.4.4+)

| Priorité | Bug | Fichier | Statut |
|----------|-----|---------|--------|
| **P0** | BUG-UPDATE-01 : Mauvais dossier d'installation | `update_checker.dart` | ✅ FIXED |
| **P0** | BUG-UPDATE-02 : `exit(0)` tue PowerShell | `update_checker.dart` | ✅ FIXED |
| **P0** | BUG-UPDATE-03 : `Get-Process` ne trouve pas le PID | `update_checker.dart` | ✅ FIXED |
| **P1** | BUG-UPDATE-04 : Checksum cassé (locale non-EN) | `update_checker.dart` | ✅ FIXED |
| **P1** | BUG-UPDATE-05 : Pas de vérification du code sortie | `update_checker.dart` | ✅ FIXED |
| **P2** | BUG-UPDATE-06 : Fichier partiel non nettoyé | `update_checker.dart` | ✅ FIXED |
| **P2** | BUG-UPDATE-07 : Comparaison pré-release | `update_checker.dart` | ✅ FIXED |
| **P3** | BUG-UPDATE-08 : Pas de fallback checksum | `update_checker.dart` | ✅ FIXED |

### Améliorations modernes ajoutées
- **Redémarrage automatique** après installation réussie
- **Notification Windows** ballon si l'installateur échoue
- Nettoyage automatique des scripts `.ps1` périmés
- Fichier temporaire nommé par PID (unique, pas de collisions)

---

## Arbre de décision

```
Bug constaté : update ZIP mais rien ne change au redémarrage
│
├─ L'utilisateur lance l'app depuis le BUILD DIRECTORY ?
│   → BUG-UPDATE-01 : l'installateur a installé dans Program Files
│   → FIX : installer dans le dossier de l'exe courant
│
├─ L'installateur a-t-il vraiment été exécuté ?
│   → BUG-UPDATE-02 : exit(0) avant que PowerShell UAC démarre
│   → FIX : synchronisation robuste
│
├─ Le processus PowerShell a-t-il trouvé l'app à fermer ?
│   → BUG-UPDATE-03 : Get-Process ne trouve pas le bon exe
│   → FIX : transmettre le PID
│
└─ L'installateur a-t-il échoué silencieusement ?
    → BUG-UPDATE-05 : pas de retour d'erreur
    → FIX : vérifier $LASTEXITCODE
```
