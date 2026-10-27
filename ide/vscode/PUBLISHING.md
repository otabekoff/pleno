# Publishing the extension

How to put the Pleno extension on the [VS Code Marketplace](https://marketplace.visualstudio.com/)
(used by VS Code) and on [Open VSX](https://open-vsx.org/) (used by VSCodium, Cursor, Windsurf,
Gitpod, Theia and other editors built on VS Code). Both take the same `.vsix` file that every
GitHub release already has (`pleno-language-vX.Y.Z.vsix`), so publish that file instead of
building a new one.

The extension's ID is `pleno.pleno-language`: the `publisher` and `name` fields of
`package.json`. On 2026-10-27 the `pleno` publisher (Marketplace) and the `pleno` namespace
(Open VSX) were both free. Changing either later changes the ID, and users would have to install
the extension again, so claim both before the first release.

## Before the first upload

- `package.json` has `publisher`, `name`, `displayName`, `version`, `engines.vscode`, `icon`,
  `repository`, `homepage`, `bugs` and `license`. `README.md`, `CHANGELOG.md` and `LICENSE` are in
  `ide/vscode`. Both stores show the README as the extension's page and the CHANGELOG as its
  changelog tab.
- The `.vsix` must not contain secrets or large files. `npx @vscode/vsce ls` lists what goes in.

## VS Code Marketplace

### Once

1. Sign in to [Azure DevOps](https://dev.azure.com/) with a Microsoft account and create an
   organization if you have none. The token below comes from there.
2. Create a personal access token at `https://dev.azure.com/<your organization>/_usersSettings/tokens`.
   You can also reach it from the person icon at the top right, then **Personal access tokens →
   New Token**.
   - **Name:** `pleno marketplace`.
   - **Organization: All accessible organizations.** A token for a single organization is
     rejected by the Marketplace.
   - **Expiration:** custom, the longest allowed (Azure DevOps allows at most a year). Set a
     reminder to renew it.
   - **Scopes: Custom defined → Show all scopes → Marketplace → Manage.** Nothing else.
   - **Create**, then copy the token. Azure DevOps shows it only once.
3. Create the publisher at
   [marketplace.visualstudio.com/manage](https://marketplace.visualstudio.com/manage/createpublisher)
   with the same Microsoft account. **ID: `pleno`**, which must match `publisher` in
   `package.json`. Name: `Pleno`.
4. Check the token:

   ```sh
   npx @vscode/vsce login pleno     # paste the token
   ```

### Each release

```sh
gh release download v0.3.0 --pattern 'pleno-language-*.vsix'
npx @vscode/vsce publish --packagePath pleno-language-v0.3.0.vsix --pat <token>
```

The version comes from the `.vsix`, and the Marketplace rejects a version it already has. The
extension's page usually appears within minutes, after an automatic scan.

## Open VSX

### Once

1. Sign in at [open-vsx.org](https://open-vsx.org/) with your GitHub account.
2. Link an Eclipse Foundation account and sign the **Open VSX Publisher Agreement** at
   [open-vsx.org/user-settings/profile](https://open-vsx.org/user-settings/profile) → **Log in
   with Eclipse**, then **Show Publisher Agreement → Agree**. Create the Eclipse account at
   [accounts.eclipse.org](https://accounts.eclipse.org/user/register) with the same email address
   as your GitHub account. Publishing fails until the agreement is signed.
3. Create an access token at [open-vsx.org/user-settings/tokens](https://open-vsx.org/user-settings/tokens):
   **Generate New Token**, with the description `pleno release`. Copy it; Open VSX shows it only
   once. It stays valid until you delete it there.
4. Create the namespace once:

   ```sh
   npx ovsx create-namespace pleno --pat <token>
   ```

5. Optional: ask for the namespace to be verified (the "verified publisher" badge) by opening an
   issue in [EclipseFdn/open-vsx.org](https://github.com/EclipseFdn/open-vsx.org/issues) that
   links the namespace to `github.com/otabekoff/pleno`.

### Each release

```sh
npx ovsx publish pleno-language-v0.3.0.vsix --pat <token>
```

## Publishing from the release workflow

The release workflow's **Publish the VS Code extension** job uploads the release's `.vsix` to both
stores on every `vX.Y.Z` tag (not on prereleases). It needs two repository secrets, which you add
at [github.com/otabekoff/pleno/settings/secrets/actions](https://github.com/otabekoff/pleno/settings/secrets/actions)
with **New repository secret**:

| Secret     | Value |
| ---------- | ----- |
| `VSCE_PAT` | The Azure DevOps token from "VS Code Marketplace → Once" |
| `OVSX_PAT` | The Open VSX token from "Open VSX → Once" |

Each store is skipped while its secret is missing. Do the first upload of each store by hand, as
above: it checks that the publisher, the namespace and the agreement are in place. When a token
expires, the job fails at that step. Make a new token and replace the secret.

## After publishing

- Update `docs/guide/editor.md`, `docs/guide/installation.md` and `ide/vscode/README.md`. They say
  the extension is installed from the `.vsix`, so give them the store links and the `ext install`
  command (`code --install-extension pleno.pleno-language`).
- `docs/community.md` says the extension is not published on purpose. Change that paragraph too.
- `scripts/dev-install.*` still installs the extension from the checkout, which is right for
  development.
