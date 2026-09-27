# Pixel Build Manager

**Get the right build of a game onto your machine in a couple of clicks, straight from GitHub Releases.**

Pixel Build Manager is a desktop app for anyone who needs to play, test or review builds that a team publishes to GitHub Releases. Instead of digging through Releases pages, downloading zips by hand, unpacking them somewhere and remembering which folder holds which version, you bind a project once. After that, the app downloads, unpacks, keeps up to date and launches its builds for you.

It was built by [Perfect Pixel Games](https://github.com/Perfect-Pixel-Games) for our own projects, and it works with any GitHub repository that publishes builds as release assets.

## What it enables

- **Teams stay on the same build.** Developers, QA and designers can grab exactly the version the team is talking about, or pin to the latest one, without asking where it is.
- **Testers and players don't need a GitHub account.** Anyone can bind a public repository by pasting its URL. They don't need to be a member of the project or to log in.
- **Private projects stay private.** Log in with GitHub to browse and sync builds from your own and your organisations' private repositories.
- **"Latest" builds keep themselves current.** Point a project at its latest release or prerelease, and new builds download and replace the old one automatically as they ship.
- **Several versions side by side.** Keep multiple releases and build configurations (for example Shipping and Development, or Windows and Linux) unpacked at once, and switch between them freely.

## Features

- **Project tabs.** Bind as many projects as you like; each gets its own tab that remembers your selections between sessions.
- **Two ways to bind a project:**
  - **Paste a GitHub URL.** Works for any public repository with releases, logged in or not. Accepts links like `https://github.com/owner/repo`, links to its `/releases` page, or the short form `owner/repo`.
  - **Pick from your account.** When logged in, browse every repository you and your organisations can access that has releases. You can favourite the ones you use most.
- **Release browser.** Search a project's releases, see which are prereleases and when they were published, and pick one. Or pick **Latest release** / **Latest prerelease** to track whatever is newest.
- **Build configs.** Each file attached to a release is a *build config*. Tick the ones you want and press **Sync**. Only what's missing is downloaded, downloads are cached and size-checked, and then they're extracted.
- **Automatic latest sync.** Projects set to a latest mode are checked every 5 minutes, and ticked configs are re-synced when a new release appears.
- **Launch and open.** Launch a synced build directly (the app finds the game's executable and skips crash handlers, redistributable installers and uninstallers), or open its folder.
- **Disk control.** **Clear cache** deletes everything a project has on disk. Logging out removes projects bound from your account, along with their files. Projects bound by URL are kept.
- **Light, dark and system themes.**
- **Self-updating.** The app checks for new versions of itself in the background and installs them.

## Getting started

1. **Install** the latest version from this repository's [Releases page](https://github.com/Perfect-Pixel-Games/pixel-build-manager/releases). Windows is supported today; download the `-setup.exe` installer.
2. **Choose a workspace folder.** On first launch, pick where builds should be stored, for example `D:\Builds`.
3. **Bind a project.** Press **+** in the tab bar, and either paste a repository URL or, once logged in, pick one from the list.
4. **Pick a release** (or **Latest release** / **Latest prerelease**), **tick the build configs** you want, and press **Sync**.
5. **Launch** the build, or open its folder.

### Logging in is optional

|                                   | Logged out                          | Logged in with GitHub               |
| --------------------------------- | ----------------------------------- | ----------------------------------- |
| Bind public repositories by URL   | ✅                                  | ✅                                  |
| Browse your repositories          | –                                   | ✅ (yours and your organisations')  |
| Private repositories              | –                                   | ✅                                  |
| GitHub API rate limit             | 60 requests/hour per network        | 5,000 requests/hour                 |

Press **Log in** in the top bar to sign in with GitHub's device flow: the app shows a short code and opens GitHub in your browser to approve it. It asks for the `repo` and `read:org` scopes so it can read private repositories' releases and list your organisations. Your token is stored in your operating system's credential store (Windows Credential Manager). It is never written to disk in plain text and never exposed to the app's web view.

Logging out removes every project you bound from your account's list, including its tabs, settings and files on disk. The app asks for confirmation first and lists what will be removed.

## Publishing builds that work well with it

Pixel Build Manager works with ordinary GitHub Releases. To get the most out of it:

- **Attach each build as its own release asset**, one per platform or configuration. Each asset becomes a separately tickable build config, for example `mygame-windows-x64-shipping.zip` and `mygame-windows-x64-development.zip`.
- **Use `.zip` or `.tar.gz` archives.** The format is detected from the file's contents, not its name.
- **Mark test builds as prereleases**, so people can choose between tracking **Latest release** and **Latest prerelease**.
- **Keep the game's executable inside the archive.** The app searches the extracted folder for it, preferring the shallowest match. It runs the executable with its own folder as the working directory, as most engines expect.
- **Keep asset names stable across versions** if people will track "latest". A ticked config is matched to each new release's assets by name. The release's tag is ignored if it appears in the name, so `mygame_1.4.0_win64.zip` and `mygame_1.5.0_win64.zip` count as the same config, but any other difference does not.

## Where builds are stored

Everything lives under the workspace folder you chose, one folder per project:

```
<workspace>/
└── <owner>/<repo>/
    ├── cache/                     downloaded archives, reused instead of re-downloading
    ├── builds/<release>/<config>/ builds synced from a specific release
    └── latest/
        ├── release/<config>/      builds tracking Latest release (always the newest one)
        └── prerelease/<config>/   builds tracking Latest prerelease
```

App settings (bound projects, selections, theme) are stored in `settings.json` in the operating system's app-data folder.

## Development

Pixel Build Manager is a [Tauri 2](https://tauri.app/) app. All logic lives in the Rust backend: GitHub access, downloading, caching, extraction, launching and settings. The React + TypeScript frontend is presentational and talks to the backend only through Tauri commands and events.

```
src/                 React + TypeScript frontend (Vite)
├── api/             typed wrappers around the backend's Tauri commands
├── components/      UI components
└── hooks/           sync and theme state
src-tauri/src/       Rust backend
├── auth/            GitHub device-flow login, token storage and refresh
├── github/          GitHub REST client and repository URL parsing
├── sync/            download, cache, extraction, launch and latest tracking
├── settings/        persisted app settings
└── updater/         self-update channel handling
docs/superpowers/    design specs and implementation plans for each feature
```

### Prerequisites

- [Node.js](https://nodejs.org/) and npm
- [Rust](https://rustup.rs/) (stable)
- The [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform (WebView2 on Windows, or the WebKitGTK and GTK development packages on Linux)

### Common commands

```sh
npm ci                       # install frontend dependencies
npm run tauri dev            # run the app with hot reload
npm run test                 # frontend tests (Vitest)
npm run build                # type-check and build the frontend

cd src-tauri
cargo test                   # backend tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Some backend tests assert Windows-style paths (such as `D:\Builds`), so they only pass on Windows, which is where CI runs.

### Branches and releases

- Work happens on `feature/*` branches, which merge into `dev` by pull request. `main` only takes merges from `dev`, or from `hotfix/*` branches.
- CI validates every pull request and push to `dev` and `main`: formatting, clippy, Rust tests, and the frontend build and tests.
- Every push to `dev` publishes a **prerelease**, and every push to `main` publishes a **release**. Versions are `0.<minor>.<patch>`, calculated from existing tags. CI builds the Windows installer, signs it for the self-updater, and publishes the update manifest the installed app reads.
- An installed app follows the channel it was built from: builds from `main` update to new releases, and builds from `dev` update to new prereleases. It checks every 15 minutes.

### Recommended IDE setup

[VS Code](https://code.visualstudio.com/) with the [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) and [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extensions.
