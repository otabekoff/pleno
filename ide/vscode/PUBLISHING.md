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

The **Publish Extension** workflow does it (see below). By hand:

```sh
gh release download v0.4.0 --pattern 'pleno-language-*.vsix'
npx @vscode/vsce publish --packagePath pleno-language-v0.4.0.vsix --pat <token>
```

The version comes from the `.vsix`, and the Marketplace rejects a version it already has. The
extension's page usually appears within minutes, after an automatic scan.

## Open VSX

Open VSX publishing needs no stored token. The **Publish Extension** workflow
(`.github/workflows/publish-extension.yml`) signs in with a short-lived OIDC token from GitHub
Actions, and Open VSX accepts it because you register that workflow as a *trusted publisher*.

### Once

1. Sign in at [open-vsx.org](https://open-vsx.org/) with your GitHub account (`otabekoff`).
2. Sign the publisher agreement:
   - Create an Eclipse Foundation account at
     [accounts.eclipse.org/user/register](https://accounts.eclipse.org/user/register).
   - At [accounts.eclipse.org/user/edit](https://accounts.eclipse.org/user/edit), set
     **GitHub Username** to `otabekoff`.
   - At [open-vsx.org/user-settings/profile](https://open-vsx.org/user-settings/profile), choose
     **Log in with Eclipse** and accept the agreement it shows.
   - That agreement is the **Eclipse Contributor Agreement (ECA)**. In its form, tick every box
     of the Developer Certificate of Origin (A to D and "I Acknowledge and Agree"). Give your
     **legal name** as in your passport, a **public name** only if it differs (for example
     `Otabek Sadiridinov`), your **employer** (`student`, `Self` or `Individual`) and your **postal
     address**, then tick **Acceptance of Terms** and press **Accept**. It has to be renewed every
     three years.
3. Create the `pleno` namespace. If [open-vsx.org/user-settings/namespaces](https://open-vsx.org/user-settings/namespaces)
   offers no button for it, use a temporary token:
   - Create it at [open-vsx.org/user-settings/tokens](https://open-vsx.org/user-settings/tokens).
   - Run `npx ovsx create-namespace pleno --pat <token>`.
   - Delete the token again on the same page.
4. Register the trusted publisher at
   [open-vsx.org/user-settings/trusted-publishers](https://open-vsx.org/user-settings/trusted-publishers),
   for the namespace `pleno`:

   | Field | Value |
   | ----- | ----- |
   | Provider | GitHub Actions |
   | Repository (owner/name) | `otabekoff/pleno` |
   | Workflow file | `publish-extension.yml` |
   | Environment | leave empty |

5. Turn the step on with a repository variable (a plain setting, not a secret):

   ```sh
   gh variable set OPEN_VSX_PUBLISH --body true --repo otabekoff/pleno
   ```

   Or create it at [github.com/otabekoff/pleno/settings/variables/actions](https://github.com/otabekoff/pleno/settings/variables/actions).
6. Optional: ask for the namespace to be verified (the "verified publisher" badge) by opening an
   issue in [EclipseFdn/open-vsx.org](https://github.com/EclipseFdn/open-vsx.org/issues) that
   links the namespace to `github.com/otabekoff/pleno`.

## Publishing from GitHub Actions

`.github/workflows/publish-extension.yml` publishes one release's `.vsix` to both stores:

- **VS Code Marketplace:** when the `VSCE_PAT` secret is set (the Azure DevOps token from "VS Code
  Marketplace → Once"). Add it with `gh secret set VSCE_PAT --repo otabekoff/pleno`, or at
  [github.com/otabekoff/pleno/settings/secrets/actions](https://github.com/otabekoff/pleno/settings/secrets/actions).
  When the token expires, the step fails with an authentication error. Make a new token and
  replace the secret.
- **Open VSX:** when `OPEN_VSX_PUBLISH` is `true`, by trusted publishing.

The release workflow starts it for every new `vX.Y.Z` tag (not for prereleases). To publish a
release that already exists, such as v0.4.0, start it by hand:

```sh
gh workflow run publish-extension.yml --repo otabekoff/pleno --ref main -f tag=v0.4.0
```

Or use **Actions → Publish Extension → Run workflow** on GitHub. Each store rejects a version it
already has, so running it twice for one tag fails at the second upload and changes nothing.

## After publishing

- Update `docs/guide/editor.md`, `docs/guide/installation.md` and `ide/vscode/README.md`. They say
  the extension is installed from the `.vsix`, so give them the store links and the `ext install`
  command (`code --install-extension pleno.pleno-language`).
- `docs/community.md` says the extension is not published on purpose. Change that paragraph too.
- `scripts/dev-install.*` still installs the extension from the checkout, which is right for
  development.
