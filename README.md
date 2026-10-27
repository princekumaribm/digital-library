# Digital Library

A complete, **offline** desk application for libraries that rent computer time. Register members, check them in and out of computers, and bill them automatically from an editable rate card.

It runs as a normal Mac app: double-click it and a small server starts on your Mac and opens the app in your web browser. No internet connection, accounts, or extra software is required. All data stays on your Mac.

---

## Table of contents

1. [Features](#features)
2. [System requirements](#system-requirements)
3. [Installation](#installation)
4. [Starting and stopping](#starting-and-stopping)
5. [Signing in and passwords](#signing-in-and-passwords)
6. [User guide](#user-guide)
7. [How billing works](#how-billing-works)
8. [Data, backup and moving to another Mac](#data-backup-and-moving-to-another-mac)
9. [Troubleshooting](#troubleshooting)
10. [Uninstalling](#uninstalling)
11. [Privacy and security](#privacy-and-security)
12. [Developer guide](#developer-guide)
13. [API reference](#api-reference)

---

## Features

| Area | What you can do |
|---|---|
| **Sign-in** | Username/password sign-in page. Change the password in Settings, or reset it from the sign-in page using the current **or** default password. Credentials are included in backups. |
| **Members** | Register members with first name, last name, email, phone and ID number (unique). Search, edit, delete, and view each member's visit history and total spend. |
| **Check-in** | Search a member (or register a new one on the spot), pick a computer and rate plan, and start the session. A member can't be checked in twice, and a computer can't be used by two people at once. |
| **Live dashboard** | See who is checked in, on which computer, with a live timer and the amount owed so far. Also shows today's revenue, check-ins, usage time and free computers. |
| **Check-out** | Shows the full bill: time used, billable time, subtotal, discount, tax and total. Pick a payment method (Cash, Card, UPI, Bank transfer, Other) and get a printable receipt. |
| **Rate card** | Create multiple plans (e.g. Standard, Student, Senior Citizen) with hourly rate, minimum charge, rounding increment and optional maximum charge. Choose the default plan, set tax % and currency, and use the built-in price calculator. |
| **History** | Every session with filters (date range, status, search), receipts, and CSV export. |
| **Reports** | Revenue, sessions, usage time, average bill, discounts and tax for any date range, broken down by day, payment method, rate plan and top members. |
| **Computers** | Add, rename, disable or remove workstations, and see at a glance which are free or in use. |
| **Settings** | Library name (shown on receipts), backup download, restore from backup, CSV exports, and stopping the app. |

---

## System requirements

- macOS 11 (Big Sur) or later.
- An Apple Silicon Mac (M1 or newer) for the `arm64` build. See [Building for Intel Macs](#building-for-intel-and-apple-silicon-universal2) if you need an Intel build.
- A web browser (Safari, Chrome, Firefox, or Edge).
- No internet connection and no Python installation needed.

---

## Installation

You can ship the app in three ways. All are in the `dist/` folder after a build.

| File | Best for | How to install |
|---|---|---|
| `DigitalLibrary-1.0.0-arm64.pkg` | Most users | Double-click and follow the installer. The app is placed in **Applications**. |
| `DigitalLibrary-1.0.0-arm64.dmg` | Classic Mac install | Open it and drag **Digital Library** onto the **Applications** shortcut. |
| `DigitalLibrary-1.0.0-arm64.zip` | Email / file sharing | Double-click to unzip, then drag **Digital Library** into **Applications**. |

### First launch on another Mac (security prompt)

The app isn't signed with an Apple Developer ID, so macOS blocks it the first time it's opened on a Mac other than the one that built it:

1. In Finder, **right-click** (or Control-click) the `.pkg` or the app and choose **Open**.
2. Click **Open** in the dialog.

On newer macOS versions, you might need to do this instead: try to open the app once, then go to **System Settings → Privacy & Security** and click **Open Anyway**.

You only need to do this once. To remove the prompt entirely, see [Signing and notarization](#signing-and-notarization).

---

## Starting and stopping

**Start:** open **Digital Library** from Applications, Launchpad or Spotlight. The app runs in the background, so there's no Dock icon, and the sign-in page opens in your browser at `http://127.0.0.1:8765/`.

- Opening the app again while it's running just opens the page again.
- You can bookmark the page. It works whenever the app is running.

**Stop:** in the web page, go to **Settings → Stop Digital Library**. You'll be warned if members are still checked in. Their sessions are kept and can be checked out after restarting, but **their time keeps counting**.

Data is saved immediately after every action, so shutting down or restarting the Mac doesn't lose data.

---

## Signing in and passwords

### Default credentials

| Username | Password |
|---|---|
| `admin` | `Admin@123` |

The username isn't case-sensitive; the password is. While the default password is in use, a yellow banner reminds you to change it.

- You stay signed in for 12 hours of inactivity. Quitting or restarting the app signs you out.
- **Log out** is at the bottom of the sidebar.
- After 5 wrong passwords in a row, sign-in is paused for 60 seconds.

### Changing the password (while signed in)

**Settings → Account & password:** enter the current password (or the default password `Admin@123`), then the new password twice, and click **Change password**. Any other open sign-ins are ended.

Password rules: at least 8 characters, including at least one letter and one number.

### Forgot the password? Reset it from the sign-in page

1. On the sign-in page, click **Forgot or want to reset the password?**
2. Enter the username (`admin`).
3. In **Current or default password**, enter either the current password **or** the default password `Admin@123`.
4. Enter and confirm the new password, then click **Reset password**.
5. Sign in with the new password.

> **Note:** because the default password `Admin@123` always works for resetting, anyone who knows it and can use this Mac can set a new password. Keep the Mac's own login password private and lock the screen when you step away.

### Credentials in backups (export and import)

- The username and password are stored in the database, so they're **exported** with **Settings → Download backup**. The password is stored as a secure hash, never as plain text.
- **Restore from backup** **imports** the credentials from the backup. Afterwards you're signed out, and you sign in with the username and password from the backup.
- Backups made before sign-in was added contain no credentials. Restoring one sets the login back to `admin` / `Admin@123`.

---

## User guide

### First-time setup

The app starts with sample settings. Adjust them before using it:

1. **Sign in** with `admin` / `Admin@123`, then change the password in **Settings → Account & password**.
2. **Settings → General:** set your **library name** (printed on receipts and the sign-in page).
3. **Rate Card → Tax & currency:** set the **tax %** and **currency symbol** (default `₹`, tax 0%).
4. **Rate Card:** edit or replace the starter plans:

   | Plan | Rate / hour | Minimum | Rounded up to |
   |---|---|---|---|
   | Standard *(default)* | ₹60 | 30 min | 15 min |
   | Student | ₹40 | 30 min | 15 min |
   | Senior Citizen | ₹30 | 15 min | 15 min |

4. **Computers:** rename or add workstations. The starters are `PC-01` to `PC-06` in "Main hall".

### Registering a member

**Members → + Add member.** All fields except notes are required:

- **First name**, **Last name**
- **Email:** must be a valid address. It's stored in lower case.
- **Phone:** digits, spaces, `+ - ( ) .` are allowed. It needs at least 6 digits.
- **ID number:** library card, student ID, national ID, etc. **Must be unique** (case-insensitive).

You can also register a new member directly from the check-in window.

### Checking in

1. Click **+ New check-in** on the Dashboard. You can also click **Check in** next to a member, or **Check in here** on a computer card.
2. Search the member by name, ID number, phone or email, and select them.
3. Choose a **computer** (only free ones are listed) and a **rate plan** (the default plan is pre-selected).
4. Optionally add a note, then click **Check in**.

The member now appears on the Dashboard with a live timer and running amount.

### Checking out

1. Click **Check out** next to the member on the Dashboard, Members, History or Computers page.
2. Review the bill. Optionally enter a **discount** (the total updates instantly) and choose the **payment method**.
3. Click **Confirm & collect**. A receipt appears; click **Print receipt** to print or save it as a PDF.

**Cancel** (on the Dashboard) ends a session **without charging**, for example if the member was checked in by mistake.

### History

All sessions, newest first. Filter by check-in date, status (active / completed / cancelled) or search text. Click **Receipt** to reprint any past receipt. **Export CSV** downloads exactly what the filters show.

### Reports

Pick a preset (Today, Last 7 days, Last 30 days, This month) or a custom date range. Reports count **completed sessions by check-out time** and show:

- Revenue, number of sessions, total and average time, average bill, discounts and tax collected.
- Breakdown by **day**, **payment method**, **rate plan**, and the **top 10 members**.

### Managing computers

- **Edit:** rename a computer, change its location, or untick *Available for check-in* to disable it.
- **Delete:** removes it. Past sessions keep the computer's name.
- A computer that is **in use** can't be disabled or deleted until the member is checked out.

### Managing rate plans

- **Edit/Add:** a live example shows what 10 min, 47 min, 1.5 h and 3 h would cost.
- **Default:** the plan pre-selected at check-in. There's always exactly one default among active plans.
- **Inactive** plans are hidden from check-in but kept for reference.
- **Changes never affect people already checked in or past receipts.** The rate, rules and tax % are locked in at check-in.

---

## How billing works

At check-out the bill is calculated as follows:

1. **Time used:** the actual time from check-in to check-out, rounded **up** to the next whole minute.
2. **Rounding:** rounded **up** to the plan's *Round up to* increment (e.g. 15 min).
3. **Minimum:** if the result is below the plan's *Minimum*, the minimum is charged.
4. **Subtotal** = billable minutes ÷ 60 × hourly rate, rounded to 2 decimals.
5. **Max charge:** if the plan has one (> 0), the subtotal is capped at it.
6. **Discount:** subtracted from the subtotal. It can't exceed the subtotal.
7. **Tax** = (subtotal − discount) × tax %.
8. **Total** = subtotal − discount + tax.

**Examples:** Standard plan, ₹60/hour, minimum 30 min, rounded up to 15 min, 18% tax.

| Time used | Billable | Subtotal | Tax (18%) | Total |
|---|---|---|---|---|
| 10 min | 30 min (minimum) | ₹30.00 | ₹5.40 | ₹35.40 |
| 47 min | 60 min | ₹60.00 | ₹10.80 | ₹70.80 |
| 1 h 31 min | 1 h 45 min | ₹105.00 | ₹18.90 | ₹123.90 |
| 3 h (max charge ₹100) | 3 h | ₹100.00 | ₹18.00 | ₹118.00 |

The amount shown on the Dashboard while a member is checked in is a live estimate. The final amount is calculated by the server at the moment of check-out.

---

## Data, backup and moving to another Mac

### Where data is stored

| What | Location |
|---|---|
| Database (members, sessions, rates, computers, settings) | `~/Library/Application Support/DigitalLibrary/library.db` |
| Safety copies made before a restore | `~/Library/Application Support/DigitalLibrary/library-before-restore-<date>.db` |
| Log file | `~/Library/Logs/DigitalLibrary.log` |

Data is **not** inside the app, so reinstalling or upgrading the app keeps your data. Sharing the `.zip`/`.pkg` does **not** share your members' data.

Tip: in Finder, press **⌘⇧G** and paste a path to open these folders.

### Backups

**Settings → Download backup** saves a complete copy of the database (e.g. `library-backup-20261027-2300.db`) to your Downloads folder. It's safe to do while the app is in use. Back up regularly and keep a copy off the Mac (USB drive or cloud storage).

**Settings → Export members / Export sessions** download CSV files that open in Excel or Numbers. These are for reporting; use the `.db` backup for restoring.

### Moving to another Mac

1. On the **old Mac**, check out everyone who is checked in.
2. On the old Mac: **Settings → Download backup**.
3. Copy the `.db` file to the new Mac (USB drive, AirDrop, etc.).
4. On the **new Mac**, install and open Digital Library.
5. On the new Mac: **Settings → Restore from backup…** and choose the file.

Everything is transferred: members, history, rate card, computers, settings, **and the login username and password**. After restoring, sign in on the new Mac with the password you used on the old Mac.

### Restoring

- **Restore from backup** replaces **all** current data with the backup.
- A safety copy of the current data is saved first (`library-before-restore-<date>.db`), so a restore can be undone by restoring that file.
- Files that aren't a valid, undamaged Digital Library backup are rejected.

**Manual method** (no app needed): stop the app (**Settings → Stop Digital Library**), then copy the backup into `~/Library/Application Support/DigitalLibrary/` and rename it to `library.db`, replacing the existing file. Also delete `library.db-wal` and `library.db-shm` if present, then start the app.

---

## Troubleshooting

| Problem | Solution |
|---|---|
| Forgot the password | On the sign-in page, click **Forgot or want to reset the password?** and use the default password `Admin@123` as the current password. |
| "Too many failed attempts" | Wait 60 seconds, then try again. |
| "Your session has ended. Please sign in again." | You were inactive for 12 hours, the app was restarted, the password was changed elsewhere, or data was restored. Sign in again. |
| Can't sign in after restoring a backup | Use the password from the Mac the backup came from, or reset it with the default password. |
| "Digital Library can't be opened because it is from an unidentified developer" | Right-click the app, choose **Open**, then **Open** again. Or use **System Settings → Privacy & Security → Open Anyway**. |
| Nothing seems to happen when I open the app | It runs in the background (no Dock icon). Look for a new browser tab, or browse to `http://127.0.0.1:8765/`. |
| Browser shows "can't connect" | The app isn't running. Open it again from Applications. |
| Page says "Cannot reach the Digital Library server" | The app was stopped. Open it again; the page reloads normally. |
| Port 8765 is used by another program | The app automatically uses the next free port (8766–8784). The browser opens the right address. Check the log for the address used. |
| Need to force-quit | Open **Activity Monitor**, search `DigitalLibrary`, and quit it. Or run `pkill -x DigitalLibrary` in Terminal. Data is safe. |
| Something went wrong | See `~/Library/Logs/DigitalLibrary.log`. |
| "A member with this ID number already exists" | ID numbers must be unique. Search for the existing member instead. |
| Can't delete a member | Members with session history can't be deleted, to keep financial records intact. Edit their details instead. |

---

## Uninstalling

1. Stop the app (**Settings → Stop Digital Library**).
2. Drag **Digital Library** from **Applications** to the Trash.
3. *Optional, deletes all data permanently:* delete `~/Library/Application Support/DigitalLibrary/`. Download a backup first if you might need it.
4. *Optional:* delete `~/Library/Logs/DigitalLibrary.log`.
5. *Optional, if installed with the `.pkg`:* run `sudo pkgutil --forget local.digitallibrary.app` in Terminal.

---

## Privacy and security

- **Offline:** the app makes no internet connections and loads no external resources (fonts, scripts, analytics).
- **Local only:** the server listens on `127.0.0.1`, so other computers on the network **can't** connect to it.
- **Sign-in required:** every data request needs a signed-in session. Passwords are hashed with PBKDF2-SHA256 (600,000 iterations, random salt). Sessions use a random token in an `HttpOnly`, `SameSite=Strict` cookie, expire after 12 hours of inactivity, and are cleared when the app restarts. Repeated wrong passwords trigger a 60-second lockout.
- **Default-password reset:** by design, the default password `Admin@123` can always be used to reset the password. This protects against being locked out, but means anyone who knows it and can use this Mac can take over the account.
- **Protected against malicious websites:** requests must come from the app's own page. The server checks the `Host` and `Origin` headers, requires JSON or binary content types (which forces browser CORS checks), and sends a strict Content-Security-Policy.
- **Safe data handling:** all database queries are parameterized, input is validated, page content is HTML-escaped, and CSV exports are protected against spreadsheet formula injection.
- **Personal data:** member details are stored unencrypted in the database file on your Mac. Protect the Mac with a login password and FileVault, and keep backup files private.

---

## Developer guide

### Technology

- **Backend:** Python standard library only (`http.server`, `sqlite3`). No third-party packages.
- **Frontend:** plain HTML, CSS and JavaScript. No frameworks, build step or CDN.
- **Database:** SQLite in WAL mode.
- **Packaging:** PyInstaller embeds Python into the `.app`. `pkgbuild`/`productbuild`, `hdiutil` and `ditto` (built into macOS) create the `.pkg`, `.dmg` and `.zip`.

### Project structure

```
.
├── library_app/
│   ├── server.py            # Backend: HTTP server, REST API, billing, SQLite
│   └── static/
│       ├── index.html       # Single-page UI
│       ├── app.js           # UI logic
│       └── app.css          # Styles (incl. receipt print styles)
├── installer/
│   ├── welcome.html         # .pkg welcome screen
│   ├── conclusion.html      # .pkg final screen
│   └── scripts/preinstall   # Stops a running copy before upgrading
├── build_release.sh         # Builds the self-contained .app + .pkg + .dmg + .zip into dist/
├── DigitalLibrary.spec      # PyInstaller configuration
├── make_icon.py             # Generates the app icon (.png / .icns)
├── build_mac_app.sh         # Quick dev build that uses the Mac's installed Python
├── Start Digital Library.command  # Run from source by double-clicking
├── build/                   # Build intermediates (safe to delete)
└── dist/                    # Release output
```

### Running from source

Requires Python 3.9+.

```bash
python3 library_app/server.py               # starts and opens the browser
python3 library_app/server.py --no-browser  # starts without opening the browser
```

You can also double-click **Start Digital Library.command**. Press `Ctrl+C` to stop.

### Configuration (environment variables)

| Variable | Default | Purpose |
|---|---|---|
| `LIBRARY_PORT` | `8765` | First port to try (the next 19 are tried if busy). |
| `LIBRARY_DATA_DIR` | `~/Library/Application Support/DigitalLibrary` | Folder for `library.db`. Use a temporary folder for testing. |
| `LIBRARY_DEBUG` | *(unset)* | Set to any value to log every HTTP request. |

Example test run with throwaway data:

```bash
LIBRARY_DATA_DIR=/tmp/dl-test LIBRARY_PORT=8799 python3 library_app/server.py --no-browser
```

### Building a release

```bash
./build_release.sh
```

This creates a build environment in `build/venv` and installs PyInstaller (the first run needs internet). It then generates the icon, builds the app, verifies its code signature, and writes to `dist/`:

- `Digital Library.app`: self-contained app (about 22 MB)
- `DigitalLibrary-<version>-<arch>.pkg`: installer to `/Applications`
- `DigitalLibrary-<version>-<arch>.dmg`: drag-to-install disk image
- `DigitalLibrary-<version>-<arch>.zip`: zipped app (created with `ditto` to preserve symlinks and signature)

Set the version with `APP_VERSION=1.1.0 ./build_release.sh`. Also update `VERSION` in `library_app/server.py`.

After any code change, run `./build_release.sh` again.

#### Building for Intel and Apple Silicon (universal2)

The default build matches the building Mac's architecture. Homebrew Python is single-architecture, so for a universal build install Python from [python.org](https://www.python.org/downloads/macos/), which is universal2, and run:

```bash
PYTHON=/usr/local/bin/python3 TARGET_ARCH=universal2 ./build_release.sh
```

If you reuse `build/venv`, delete it first so it's recreated with the new Python.

#### Signing and notarization

To avoid the security prompt on other Macs, you need an Apple Developer account with **Developer ID Application** and **Developer ID Installer** certificates:

```bash
SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
INSTALLER_SIGN_IDENTITY="Developer ID Installer: Your Name (TEAMID)" \
./build_release.sh

xcrun notarytool submit dist/DigitalLibrary-1.0.0-arm64.pkg \
  --apple-id you@example.com --team-id TEAMID --password <app-specific-password> --wait
xcrun stapler staple dist/DigitalLibrary-1.0.0-arm64.pkg
```

#### Quick development build

`./build_mac_app.sh` creates a lightweight `Digital Library.app` in the project folder that runs the source with the Mac's installed Python. `--install` also copies it to `~/Applications`. Use it only for development; it doesn't work on Macs without Python.

### Automatic builds (GitHub Actions)

[.github/workflows/build.yml](.github/workflows/build.yml) rebuilds the app on a GitHub macOS runner for every push to any branch, every pull request, or manually (**Actions → Build macOS app → Run workflow**).

Each run:

1. Builds the `.app`, `.pkg`, `.dmg` and `.zip` with `./build_release.sh`.
2. Smoke-tests the built app: it starts the app, loads the page, checks that data requires sign-in, signs in with the default credentials, and stops the app.
3. Uploads the `.pkg`, `.dmg` and `.zip` as a downloadable **artifact**. Open the run on the **Actions** tab, then **Artifacts**. Artifacts are kept for 30 days.

**Publishing a release:** push a version tag, and the workflow also creates a GitHub Release with the installers attached. The version number comes from the tag.

```bash
git tag v1.0.1
git push origin v1.0.1
```

### Database schema

| Table | Purpose | Key columns |
|---|---|---|
| `users` | Login credentials | `username` (unique, case-insensitive), `password_hash` (`pbkdf2_sha256$iterations$salt$hash`), `default_password` (1 while the default password is in use) |
| `members` | Library members | `first_name`, `last_name`, `email`, `phone`, `id_number` (unique, case-insensitive), `notes` |
| `computers` | Workstations | `name` (unique), `location`, `active` |
| `rate_plans` | Rate card | `name` (unique), `rate_per_hour`, `minimum_minutes`, `increment_minutes`, `max_charge`, `is_default`, `active` |
| `sessions` | Check-ins/check-outs | `member_id`, `computer_id`, `check_in`, `check_out`, `status` (`active`/`completed`/`cancelled`), billing results (`duration_minutes`, `billable_minutes`, `subtotal`, `discount`, `tax`, `total`, `payment_method`) |
| `settings` | Key/value settings | `library_name`, `currency`, `tax_percent` |

- Each session stores a **snapshot** of the plan and tax at check-in (`plan_name`, `rate_per_hour`, `minimum_minutes`, `increment_minutes`, `max_charge`, `tax_percent`, `computer_name`). Later rate-card changes therefore never alter running or past sessions.
- Partial unique indexes allow only **one active session per member** and **per computer**.
- Times are stored as UTC ISO-8601 (`2026-10-27T17:30:00Z`) and shown in the Mac's local time.
- Tables are created automatically on startup (`CREATE TABLE IF NOT EXISTS`). Starter data is inserted only on the very first run.

---

## API reference

All endpoints are served at `http://127.0.0.1:<port>`.

- Request and response bodies are JSON, unless noted otherwise.
- `POST`/`PUT` requests must send `Content-Type: application/json`.
- All endpoints except `/api/health` and `/api/auth/*` require a signed-in session: the `dl_session` cookie from `/api/auth/login`.
- Errors return `{"error": "message"}` with an HTTP status: `400` invalid input, `401` not signed in or wrong password, `404` not found, `409` conflict, `415` wrong content type, `429` too many failed sign-ins.

| Method | Path | Description |
|---|---|---|
| GET | `/api/health` | `{"app": "digital-library", "version": "..."}` |
| GET | `/api/auth/status` | `{"authenticated", "username", "default_password", "library_name"}` |
| POST | `/api/auth/login` | `username`, `password`. Sets the session cookie |
| POST | `/api/auth/logout` | Ends the session |
| POST | `/api/auth/change-password` | `current_password` (current **or** default), `new_password`, plus `username` when not signed in. Ends other sessions |
| GET | `/api/dashboard` | Live counts and today's totals |
| GET / PUT | `/api/settings` | Read / update `library_name`, `currency`, `tax_percent` |
| GET | `/api/members?q=&limit=` | List / search members (includes `visits`, `total_spent`, `active_session_id`) |
| POST | `/api/members` | Create member: `first_name`, `last_name`, `email`, `phone`, `id_number`, `notes` |
| GET / PUT / DELETE | `/api/members/{id}` | Read / update / delete (delete fails if the member has sessions) |
| GET / POST | `/api/computers` | List / create: `name`, `location`, `active` |
| PUT / DELETE | `/api/computers/{id}` | Update / delete (fails while in use) |
| GET / POST | `/api/rates` | List / create plan: `name`, `rate_per_hour`, `minimum_minutes`, `increment_minutes`, `max_charge`, `description`, `active`, `is_default` |
| PUT / DELETE | `/api/rates/{id}` | Update / delete plan |
| GET | `/api/sessions?status=&from=&to=&member_id=&q=&limit=` | List sessions (`from`/`to` are ISO dates, filter on check-in time) |
| GET | `/api/sessions/{id}` | One session (used for receipts) |
| POST | `/api/sessions/checkin` | `member_id`, optional `computer_id`, `rate_plan_id` (default plan if omitted), `notes` |
| GET | `/api/sessions/{id}/quote?discount=` | Bill preview for an active session |
| POST | `/api/sessions/{id}/checkout` | `payment_method`, optional `discount`, `notes`. Returns the completed session |
| POST | `/api/sessions/{id}/cancel` | End without charge |
| GET | `/api/reports?from=&to=` | Summary and breakdowns (filter on check-out time) |
| GET | `/api/export/members.csv` | Members CSV |
| GET | `/api/export/sessions.csv?…` | Sessions CSV (same filters as `/api/sessions`) |
| GET | `/api/backup` | Download the SQLite database |
| POST | `/api/restore` | Upload a backup (`Content-Type: application/octet-stream`). Replaces all data, including credentials, after saving a safety copy, then signs everyone out |
| POST | `/api/shutdown` | Stop the server |
