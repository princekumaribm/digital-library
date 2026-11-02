# Digital Library

A complete, **offline** desk application for libraries that rent computer time. Register members, check them in and out of computers, and bill them automatically from an editable rate card.

It's a native desktop app for **macOS and Windows**: double-click it and it opens in its own window. No internet connection, web browser, accounts, or extra software is required. All data stays on your computer.

---

## Table of contents

1. [Features](#features)
2. [System requirements](#system-requirements)
3. [Installation](#installation)
4. [Starting and stopping](#starting-and-stopping)
5. [Signing in and passwords](#signing-in-and-passwords)
6. [User guide](#user-guide)
7. [How billing works](#how-billing-works)
8. [Data, backup and moving to another computer](#data-backup-and-moving-to-another-computer)
9. [Troubleshooting](#troubleshooting)
10. [Uninstalling](#uninstalling)
11. [Privacy and security](#privacy-and-security)
12. [Developer guide](#developer-guide)
13. [Internal API reference](#internal-api-reference)

---

## Features

| Area | What you can do |
|---|---|
| **Sign-in** | Username/password sign-in page. Change the password in Settings, or reset it from the sign-in page using the current **or** default password. Credentials are included in backups. |
| **Members** | Register members with first name, last name, email, phone and ID number (unique). Search, edit, delete, and view each member's visit history and total spend. |
| **Check-in** | Search a member (or register a new one on the spot) and start the session. The device defaults to **Own laptop**; click **Change** to pick a library computer. A member can't be checked in twice, and a computer can't be used by two people at once. |
| **Monthly passes** | Sell passes such as "1 hr/day" or "2 hrs/day" for 30 days. Each pass is a pool of hours (hours per day × days) usable on any day. See hours used and left per member. The rate plan's minimum (e.g. 30 min) and rounding also apply to pass usage; time the pass can't cover is charged at the normal rate. |
| **Live dashboard** | See who is checked in, on which computer, with a live timer and the amount owed so far. Also shows today's revenue, check-ins, usage time and free computers. |
| **Check-out** | Shows the full bill: time used, billable time, subtotal, discount, tax and total. Pick a payment method (Cash, Card, UPI, Bank transfer, Other) and get a printable receipt. |
| **Rate card** | Create multiple plans (e.g. Standard, Student, Senior Citizen) with hourly rate, minimum charge, rounding increment and optional maximum charge. Choose the default plan, set tax % and currency, and use the built-in price calculator. |
| **History** | Every session with filters (date range, status, search), receipts, and CSV export. |
| **Reports** | Revenue, sessions, usage time, average bill, discounts and tax for any date range, broken down by day, payment method, rate plan and top members. Voided pass sales are listed with their reason, date and time. |
| **Daily summary** | A closing report for any day: totals, totals by payment method, every transaction, and voided passes. Print it, save it as PDF, or save it as CSV. |
| **Computers** | Add, rename, disable or remove workstations, and see at a glance which are free or in use. |
| **Settings** | Library name, address, phone and email (printed on receipts with the logo), password change, save and restore backups, and CSV exports. |

---

## System requirements

- **macOS** 11 (Big Sur) or later, on Apple Silicon or Intel. The GitHub-built app is universal.
- **Windows** 10 or 11 (64-bit). The app uses Microsoft Edge WebView2, which is built into Windows 11 and current Windows 10. If it's missing, the installer downloads it once; that's the only time internet may be needed.
- No web browser, Python or other software needed.

---

## Installation

Installers are produced in `dist/` by a build, and attached to each [GitHub Release](https://github.com/princekumaribm/digital-library/releases).

**macOS**

| File | Best for | How to install |
|---|---|---|
| `DigitalLibrary-<version>-macos-universal.pkg` | Most users | Double-click and follow the installer. The app is placed in **Applications**. |
| `DigitalLibrary-<version>-macos-universal.dmg` | Classic Mac install | Open it and drag **Digital Library** onto the **Applications** shortcut. |
| `DigitalLibrary-<version>-macos-universal.zip` | Email / file sharing | Double-click to unzip, then drag **Digital Library** into **Applications**. |

**Windows**

| File | Best for | How to install |
|---|---|---|
| `DigitalLibrary-<version>-windows-x64-setup.exe` | Most users | Double-click and follow the installer. No admin rights needed. Adds Start menu and desktop shortcuts, and an uninstaller. |
| `DigitalLibrary-<version>-windows-x64.msi` | IT departments / managed PCs | Install with a double-click or with deployment tools (`msiexec /i …`). |
| `DigitalLibrary-<version>-windows-x64-portable.zip` | No installation | Unzip and run `DigitalLibrary.exe`. |

### First launch (security prompt)

The app isn't signed with a paid code-signing certificate, so the system warns the first time it's opened.

**macOS:**
1. In Finder, **right-click** (or Control-click) the `.pkg` or the app and choose **Open**.
2. Click **Open** in the dialog.

On newer macOS versions, you might need to do this instead: try to open the app once, then go to **System Settings → Privacy & Security** and click **Open Anyway**.

**Windows:** if **"Windows protected your PC"** (SmartScreen) appears, click **More info → Run anyway**.

You only need to do this once. To remove the prompts entirely, see [Signing and notarization](#signing-and-notarization).

---

## Starting and stopping

**Start:** open **Digital Library** from Applications, Launchpad or Spotlight (macOS), or from the Start menu or desktop shortcut (Windows). It opens in its own window at the sign-in page.

- Opening it again while it's running brings the existing window to the front. Only one copy runs at a time.

**Quit:** close the window, or press **⌘Q** on macOS. Members who are still checked in stay checked in and can be checked out after reopening, but **their time keeps counting**.

Data is saved immediately after every action, so quitting or restarting the computer doesn't lose data.

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

> **Note:** because the default password `Admin@123` always works for resetting, anyone who knows it and can use this computer can set a new password. Keep the computer's own login password private and lock the screen when you step away.

### Credentials in backups (export and import)

- The username and password are stored in the database, so they're **exported** with **Settings → Save backup…**. The password is stored as a secure hash, never as plain text.
- **Restore from backup** **imports** the credentials from the backup. Afterwards you're signed out, and you sign in with the username and password from the backup.
- Backups made before sign-in was added contain no credentials. Restoring one sets the login back to `admin` / `Admin@123`.

---

## User guide

### First-time setup

The app starts with sample settings. Adjust them before using it:

1. **Sign in** with `admin` / `Admin@123`, then change the password in **Settings → Account & password**.
2. **Settings → Library details:** check the **library name**, **address**, **phone** and **email**. They're printed on every receipt under the logo. Defaults: Shivam Digital Library, Ordely Bazar, Varanasi, ssetpamgs@gmail.com.
3. **Rate Card → Tax & currency:** set the **tax %** and **currency symbol** (default `₹`, tax 0%).
4. **Rate Card:** edit or replace the starter plans:

   | Plan | Rate / hour | Minimum | Rounded up to |
   |---|---|---|---|
   | Standard *(default)* | ₹60 | 30 min | 15 min |
   | Student | ₹40 | 30 min | 15 min |
   | Senior Citizen | ₹30 | 15 min | 15 min |

5. **Passes → Pass plans:** set your monthly pass prices. The starters are "Monthly – 1 hr/day" (₹600) and "Monthly – 2 hrs/day" (₹1,000), both valid for 30 days.
6. **Computers:** rename or add workstations. The starters are `PC-01` to `PC-06` in "Main hall". Members using their own laptop don't need a computer.

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
3. If the member has a valid pass with hours left, **Use pass** is ticked automatically, showing the hours left and the expiry date. Untick it to bill this visit normally.
4. **Device** is **Own laptop** by default. If the member uses a library computer, click **Change** and pick one (only free computers are listed).
5. Check the **rate plan** (the default is pre-selected). It's used for normal billing, and for any time beyond the pass.
6. Optionally add a note, then click **Check in**.

The member now appears on the Dashboard with a live timer and running amount. Pass members also show their hours left, counting down.

### Checking out

1. Click **Check out** next to the member on the Dashboard, Members, History or Computers page.
2. Review the bill. Optionally enter a **discount** (the total updates instantly) and choose the **payment method**.
3. Click **Confirm & collect**. A receipt appears; click **Print receipt** to print or save it as a PDF.

**Cancel** (on the Dashboard) ends a session **without charging**, for example if the member was checked in by mistake.

### Monthly passes

A pass gives a member a **pool of hours** for a period, by default 30 days from the start date:

| Pass | Hours in the pool | Example use |
|---|---|---|
| 1 hr/day × 30 days | 30 hours | 3 hours on Monday, none on Tuesday, … any mix up to 30 hours |
| 2 hrs/day × 30 days | 60 hours | Any mix up to 60 hours |

**Selling a pass:** go to **Passes → + Sell pass**, or click **Sell pass** next to a member, or use **Sell a pass** in the check-in window. Pick the member, the pass, the start date, an optional discount and the payment method. A pass receipt with the logo and address appears and can be printed.

**Using a pass:** at check-in, **Use pass** is ticked automatically. At check-out:

- Time used is taken from the pass first.
- The visit's **counted time** is worked out exactly as for a normal visit: the plan's **minimum** (e.g. 30 min) and **rounding** (e.g. up to 15 min) apply. A 10-minute visit therefore counts as 30 minutes and uses 30 minutes of the pass.
- The counted time is taken from the pass first. Anything the pass can't cover is charged at the member's rate plan.
- The receipt shows the counted time, the time taken from the pass, the charged time, and the **pass hours left**.

**Tracking:**

- **Passes** lists every pass with a usage bar (hours used of total), hours left, validity dates and status. You can filter by status and search.
- **Members** shows each member's current pass and hours left.
- A member's **History** lists their passes and sessions.

| Status | Meaning |
|---|---|
| Active | Valid today and has hours left |
| Upcoming | Start date is in the future |
| Used up | All hours used |
| Expired | The end date has passed. Unused hours lapse. |
| Voided | Voided with a reason; kept on record but not counted in revenue |

- A pass can be **voided** only if it hasn't been used yet, for example if it was sold to the wrong member. Click **Void**, enter a **reason** (required) and confirm. The pass isn't deleted: it stays in the Passes list (filter **Voided**), on the member's history and in **Reports → Voided pass sales** with the amount, the date and time it was voided and the reason. Its receipt is stamped **VOIDED**. Voiding cannot be undone.
- If a member has more than one valid pass, the one that expires first is used first.
- **Pass plans** (at the bottom of the Passes page) set the name, hours per day, number of days and price. Changing a plan doesn't affect passes already sold.
- Pass sales count toward revenue on the Dashboard and in Reports.

### Receipts

Every receipt (computer usage and pass) is printed with the **logo**, **library name**, **address**, **phone** and **email** from **Settings → Library details**.

### History

All sessions, newest first. Filter by check-in date, status (active / completed / cancelled) or search text. Click **Receipt** to reprint any past receipt. **Export CSV** downloads exactly what the filters show.

### Reports

Pick a preset (Today, Last 7 days, Last 30 days, This month) or a custom date range. Reports count **completed sessions by check-out time** and show:

- Revenue (sessions and pass sales), pass sales, number of sessions, total and average time, average session bill, discounts and tax collected.
- Breakdown by **day**, **payment method**, **rate plan** (passes appear as "Pass: …"), and the **top 10 members**.
- **Voided passes**: count and amount (not included in revenue), plus a **Voided pass sales** table with pass number, member, amount, when it was sold, when it was voided and the reason.

### Daily summary (closing report)

At closing time, click **Day summary** on the Dashboard (or on Reports) to get the day's record. Pick another date to see an earlier day. The summary shows:

- Sessions and time used, session income, passes sold and pass income, discounts, tax and the **total collected**.
- Totals **by payment method** (Cash, UPI, Card, …), to match against the cash drawer and UPI statement.
- Every session checked out and every pass sold that day, with times, members and amounts.
- Cancelled sessions and **voided passes** (with reasons), which are not counted in the total.
- Lines for *Cash counted*, *Closed by* and *Signature*.

Keep a record outside the app:

- **Print / Save PDF** prints it on paper, or choose **Save as PDF** in the print dialog to keep a file.
- **Save CSV** saves a spreadsheet file (`daily-summary-YYYY-MM-DD.csv`) that opens in Excel or Numbers.

The summary uses the same rules as Reports: sessions count on their check-out day, passes on their sale day, and voided passes on the day they were voided.

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

The amount shown on the Dashboard while a member is checked in is a live estimate. The final amount is calculated by the app at the moment of check-out.

**With a monthly pass:** steps 1–3 work the same way, giving the visit's counted time, including the minimum. The counted time is then taken from the pass, and only what the pass can't cover is charged (steps 4–8).

| Visit (Standard: min 30, rounded up to 15) | Pass hours left | Taken from pass | Charged |
|---|---|---|---|
| 10 min | 50h | 30 min | ₹0 |
| 47 min | 50h | 60 min | ₹0 |
| 10 min | 20 min | 20 min | 10 min = ₹10 |
| 10 min | 0 (used up) | – | 30 min = ₹30 |

---

## Data, backup and moving to another computer

### Where data is stored

| What | macOS | Windows |
|---|---|---|
| Database (members, sessions, rates, computers, settings, login) | `~/Library/Application Support/DigitalLibrary/library.db` | `%APPDATA%\DigitalLibrary\library.db` |
| Safety copies made before a restore | same folder, `library-before-restore-<date>.db` | same folder |

The macOS location is the same as the earlier browser-based version, so existing data opens automatically in the desktop app.

Data is **not** inside the app, so reinstalling or upgrading the app keeps your data. Sharing the installers does **not** share your members' data.

Tip: in Finder, press **⌘⇧G** and paste the path. In Windows Explorer, paste `%APPDATA%\DigitalLibrary` into the address bar.

### Backups

**Settings → Save backup…** asks where to save a complete copy of the database (e.g. `library-backup-20261027-2300.db`). It's safe to do while the app is in use. Back up regularly and keep a copy off the computer (USB drive or cloud storage).

**Settings → Export members / Export sessions** (and **Export CSV** on the Members and History pages) save CSV files that open in Excel or Numbers. These are for reporting; use the `.db` backup for restoring.

### Moving to another computer

Backups work across platforms: a backup from a Mac can be restored on Windows, and the other way round.

1. On the **old computer**, check out everyone who is checked in.
2. On the old computer: **Settings → Save backup…**.
3. Copy the `.db` file to the new computer (USB drive, AirDrop, network share, etc.).
4. On the **new computer**, install and open Digital Library.
5. On the new computer: **Settings → Restore from backup…** and choose the file.

Everything is transferred: members, history, rate card, computers, settings, **and the login username and password**. After restoring, sign in with the password you used on the old computer.

### Restoring

- **Restore from backup** replaces **all** current data with the backup.
- A safety copy of the current data is saved first (`library-before-restore-<date>.db`), so a restore can be undone by restoring that file.
- Files that aren't a valid, undamaged Digital Library backup are rejected.

**Manual method** (no app needed): quit the app, then copy the backup into the data folder (see the table above) and rename it to `library.db`, replacing the existing file. Also delete `library.db-wal` and `library.db-shm` if present, then start the app.

---

## Troubleshooting

| Problem | Solution |
|---|---|
| Forgot the password | On the sign-in page, click **Forgot or want to reset the password?** and use the default password `Admin@123` as the current password. |
| "Too many failed attempts" | Wait 60 seconds, then try again. |
| "Your session has ended. Please sign in again." | You were inactive for 12 hours, the app was restarted, the password was changed elsewhere, or data was restored. Sign in again. |
| Can't sign in after restoring a backup | Use the password from the computer the backup came from, or reset it with the default password. |
| "Digital Library can't be opened because it is from an unidentified developer" (macOS) | Right-click the app, choose **Open**, then **Open** again. Or use **System Settings → Privacy & Security → Open Anyway**. |
| "Windows protected your PC" | Click **More info → Run anyway**. |
| Windows: the window is blank or the app doesn't start | Microsoft Edge WebView2 is missing. Run the `setup.exe` installer once while online, or install "WebView2 Runtime" from Microsoft. |
| Opening the app again doesn't open a second window | That's intended. Only one copy runs, and its window is brought to the front. |
| Need to force-quit | macOS: **Activity Monitor**, search `DigitalLibrary`, and quit it. Windows: **Task Manager**, end `DigitalLibrary.exe`. Data is safe. |
| "A member with this ID number already exists" | ID numbers must be unique. Search for the existing member instead. |
| Can't delete a member | Members with session history can't be deleted, to keep financial records intact. Edit their details instead. |

---

## Uninstalling

**macOS**
1. Quit the app.
2. Drag **Digital Library** from **Applications** to the Trash.
3. *Optional, deletes all data permanently:* delete `~/Library/Application Support/DigitalLibrary/`. Save a backup first if you might need it.
4. *Optional, if installed with the `.pkg`:* run `sudo pkgutil --forget com.digitallibrary.desktop` in Terminal.

**Windows**
1. Quit the app.
2. **Settings → Apps → Installed apps → Digital Library → Uninstall**. For the portable zip, just delete the folder.
3. *Optional, deletes all data permanently:* delete `%APPDATA%\DigitalLibrary`. Save a backup first if you might need it.

---

## Privacy and security

- **Offline and no network port:** the app makes no internet connections, loads no external resources, and doesn't listen on any network port. The window talks to the app's built-in logic directly, so nothing else on the computer or network can connect to it.
- **Locked-down window:** a strict Content-Security-Policy allows only the app's own files. The window may only call the app's own commands; all other system access is disabled. File save and open dialogs are opened by the app itself, never by the page.
- **Sign-in required:** every data action needs a signed-in user. Passwords are hashed with PBKDF2-SHA256 (600,000 iterations, random salt). Sign-in lasts until you log out, quit the app, or are inactive for 12 hours. Repeated wrong passwords trigger a 60-second lockout.
- **Default-password reset:** by design, the default password `Admin@123` can always be used to reset the password. This protects against being locked out, but means anyone who knows it and can use this computer can take over the account.
- **Safe data handling:** all database queries are parameterized, input is validated, page content is HTML-escaped, and CSV exports are protected against spreadsheet formula injection.
- **Personal data:** member details are stored unencrypted in the database file on your computer. Protect the computer with a login password and disk encryption (FileVault on macOS, BitLocker on Windows), and keep backup files private.

---

## Developer guide

### Technology

- **App shell:** [Tauri 2](https://tauri.app). It uses the system's built-in web engine: WebKit on macOS, Edge WebView2 on Windows. That's why the app is only a few MB.
- **Backend:** Rust (`src-tauri/`), with SQLite (bundled `rusqlite`) and PBKDF2 password hashing. The database format and password hashes are compatible with the earlier Python version.
- **Frontend:** plain HTML, CSS and JavaScript (`ui/`). No frameworks, bundler or CDN.
- **Packaging:** Tauri builds the `.app`, `.dmg`, NSIS `setup.exe` and `.msi`. `scripts/make_pkg.sh` wraps the Mac app in a `.pkg`.

### Project structure

```
.
├── ui/                      # The app's screens (loaded into the native window)
│   ├── index.html
│   ├── app.js               # UI logic; calls the backend via Tauri's invoke()
│   └── app.css              # Styles (incl. receipt print styles)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs          # App start-up, window, single-instance, the `api` command
│   │   ├── routes.rs        # All features: sign-in, members, sessions, billing, reports, files (+ tests)
│   │   ├── db.rs            # SQLite schema, start-up, query helpers
│   │   ├── util.rs          # Validation, time, money rounding, password hashing
│   │   └── error.rs         # Error type returned to the UI
│   ├── tauri.conf.json      # App name, version, window, security policy, installer settings
│   ├── capabilities/        # What the window is allowed to call
│   ├── icons/               # Generated app icons (Mac .icns, Windows .ico, PNGs)
│   └── Cargo.toml           # Rust dependencies
├── scripts/
│   ├── build.sh             # Builds the app and collects installers into dist/
│   └── make_pkg.sh          # Wraps the Mac app in a .pkg installer
├── installer/               # .pkg welcome/finish screens and pre-install script
├── assets/logo.png          # Original Shivam Digital Library logo
├── scripts/make_icons.py    # Builds ui/logo.png and the app icon from assets/logo.png
├── package.json             # Tauri command-line tool
└── .github/workflows/build.yml   # Automatic Mac + Windows builds
```

### Prerequisites

- [Rust](https://rustup.rs) (stable) and [Node.js](https://nodejs.org) 20+.
- macOS: Xcode Command Line Tools (`xcode-select --install`).
- Windows: Microsoft C++ Build Tools ("Desktop development with C++") and WebView2 (preinstalled on Windows 10/11).

Then install the Tauri command-line tool once:

```bash
npm install
```

### Running during development

```bash
npm run dev
```

This opens the app with live reload of the files in `ui/`. To use throwaway data instead of your real database, set `LIBRARY_DATA_DIR`:

```bash
LIBRARY_DATA_DIR=/tmp/dl-test npm run dev
```

### Tests

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml
```

The tests check:

- billing results, which must match the original Python version exactly
- money rounding
- that password hashes created by the Python version are accepted
- a full desk workflow: sign-in, members, check-in and check-out, reports, the rate card, and password change and reset
- the lockout after failed sign-ins
- backup restore, including imported credentials and rejected files

### Building a release

```bash
npm run build                                   # this computer's platform
TARGET=universal-apple-darwin npm run build     # macOS, Intel + Apple Silicon (rustup target add x86_64-apple-darwin first)
APP_VERSION=1.1.0 npm run build                 # override the version
```

Installers are written to `dist/`:

- macOS: `.dmg`, `.pkg`, `.zip`
- Windows: `-setup.exe`, `.msi`, `-portable.zip`

The version comes from `src-tauri/tauri.conf.json`. It's also shown in **Settings → About**. Windows installers can only be built on Windows, which the GitHub workflow does for you.

#### Signing and notarization

Without paid certificates, users see a one-time security prompt (see [First launch](#first-launch-security-prompt)). To remove it:

- **macOS:** you need an Apple Developer account with **Developer ID Application** and **Developer ID Installer** certificates. Set `bundle.macOS.signingIdentity` in `tauri.conf.json` and `INSTALLER_SIGN_IDENTITY` for the `.pkg`, then notarize:
  ```bash
  xcrun notarytool submit dist/DigitalLibrary-<version>-macos-universal.pkg \
    --apple-id you@example.com --team-id TEAMID --password <app-specific-password> --wait
  xcrun stapler staple dist/DigitalLibrary-<version>-macos-universal.pkg
  ```
- **Windows:** buy a code-signing certificate (OV or EV) and configure `bundle.windows.certificateThumbprint` (or `signCommand`) in `tauri.conf.json`.

### Automatic builds (GitHub Actions)

[.github/workflows/build.yml](.github/workflows/build.yml) runs for every push to any branch, every pull request, or manually (**Actions → Build desktop app → Run workflow**). It builds on two machines in parallel:

| Runner | Builds |
|---|---|
| macOS | Universal (Intel + Apple Silicon) `.dmg`, `.pkg`, `.zip` |
| Windows | `setup.exe`, `.msi`, portable `.zip` |

Each runner first runs the backend tests, then builds and uploads its installers as a downloadable **artifact**. Open the run on the **Actions** tab, then **Artifacts**. Artifacts are kept for 30 days.

**Publishing a release:** push a version tag. When both builds succeed, a GitHub Release is created with all the Mac and Windows installers attached. The version shown in the app and in file names comes from the tag.

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
| `settings` | Key/value settings | `library_name`, `library_address`, `library_phone`, `library_email`, `currency`, `tax_percent` |
| `pass_plans` | Monthly pass offers | `name`, `hours_per_day`, `validity_days`, `price`, `active` |
| `member_passes` | Passes sold | `member_id`, `plan_name`, `total_minutes`, `start_date`, `end_date` (local dates, inclusive), `price`, `discount`, `tax`, `total`, `payment_method`, `status` (`active`/`cancelled`; `cancelled` means voided), `void_reason`, `voided_at` |
| `pass_status` *(view)* | Pass usage | All `member_passes` columns plus `used_minutes` (sum of `sessions.pass_minutes`) |

- Each session stores a **snapshot** of the plan and tax at check-in (`plan_name`, `rate_per_hour`, `minimum_minutes`, `increment_minutes`, `max_charge`, `tax_percent`, `computer_name`). Later rate-card changes therefore never alter running or past sessions.
- Partial unique indexes allow only **one active session per member** and **per computer**.
- Times are stored as UTC ISO-8601 (`2026-10-27T17:30:00Z`) and shown in the computer's local time.
- Tables are created automatically on startup (`CREATE TABLE IF NOT EXISTS`). Starter data is inserted only on the very first run.

---

## Internal API reference

There's no web server. The UI calls a single Tauri command, `api`, with a method, a path and an optional JSON body. The paths are the same as in the earlier HTTP version:

```js
await window.__TAURI__.core.invoke("api", { method: "GET", path: "/api/members?q=asha", body: null });
```

- On success the command returns JSON.
- On failure it rejects with `{status, error}`. The status codes follow HTTP conventions: `400` invalid input, `401` not signed in or wrong password, `404` not found, `409` conflict, `429` too many failed sign-ins.
- Everything except `/api/health` and `/api/auth/*` requires a signed-in user.

| Method | Path | Description |
|---|---|---|
| GET | `/api/health` | `{"app": "digital-library", "version": "..."}` |
| GET | `/api/auth/status` | `{"authenticated", "username", "default_password", "library_name"}` |
| POST | `/api/auth/login` | `username`, `password` |
| POST | `/api/auth/logout` | Signs out |
| POST | `/api/auth/change-password` | `current_password` (current **or** default), `new_password`, plus `username` when not signed in |
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
| POST | `/api/sessions/checkin` | `member_id`, optional `computer_id` (omit for **Own laptop**), `rate_plan_id` (default plan if omitted), `use_pass` (use the member's current pass), `notes` |
| GET | `/api/sessions/{id}/quote?discount=` | Bill preview for an active session |
| POST | `/api/sessions/{id}/checkout` | `payment_method`, optional `discount`, `notes`. Returns the completed session |
| POST | `/api/sessions/{id}/cancel` | End without charge |
| GET | `/api/reports?from=&to=` | Summary and breakdowns (sessions by check-out time, pass sales by sale time) plus `voided` passes (by void time) |
| GET | `/api/daily-summary?date=YYYY-MM-DD` | Closing report for one local day (default today): `summary`, `by_payment`, `sessions`, `passes`, `voided` |
| GET / POST | `/api/pass-plans` | List / create pass plan: `name`, `hours_per_day`, `validity_days`, `price`, `description`, `active` |
| PUT / DELETE | `/api/pass-plans/{id}` | Update / delete pass plan (sold passes keep their own copy) |
| GET | `/api/passes?member_id=&q=&state=` | List passes with `used_minutes`, `remaining_minutes` and `state` (active/upcoming/used up/expired/voided) |
| POST | `/api/passes` | Sell: `member_id`, `pass_plan_id`, optional `start_date` (YYYY-MM-DD), `discount`, `payment_method`, `notes` |
| GET | `/api/passes/{id}` | One pass (used for pass receipts) |
| POST | `/api/passes/{id}/void` | Void an unused pass: `reason` (required, 3–200 characters). The pass is kept with `void_reason` and `voided_at` |
| POST | `/api/export/passes?…` | Save passes CSV |
| POST | `/api/export/daily-summary?date=` | Save the daily summary CSV |
| POST | `/api/export/members` | Shows a Save dialog and writes the members CSV. Returns `{saved}` or `{cancelled}` |
| POST | `/api/export/sessions?…` | Same for sessions (same filters as `/api/sessions`) |
| POST | `/api/backup` | Shows a Save dialog and writes a copy of the database |
| POST | `/api/restore` | Shows an Open dialog, validates the file, saves a safety copy, replaces all data (including credentials), then signs out |
| POST | `/api/print` | Opens the system print dialog for the receipt |
