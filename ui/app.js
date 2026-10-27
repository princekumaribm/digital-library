"use strict";

const state = {
  settings: { library_name: "Digital Library", currency: "₹", tax_percent: 0, payment_methods: ["Cash"] },
  rates: [],
  computers: [],
  members: [],
  active: [],
  history: [],
  clockOffset: 0,
  auth: { authenticated: false },
};

// ------------------------------------------------------------ helpers

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const ESC = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
const esc = (v) => String(v ?? "").replace(/[&<>"']/g, (c) => ESC[c]);
const pad = (n) => String(n).padStart(2, "0");
const round2 = (n) => Math.round((Number(n) + Number.EPSILON) * 100) / 100;
const fullName = (o) => `${o.first_name} ${o.last_name}`;

function money(v) {
  return state.settings.currency + Number(v || 0).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
function fmtMinutes(min) {
  min = Math.max(0, Math.round(Number(min) || 0));
  const h = Math.floor(min / 60), m = min % 60;
  return h ? (m ? `${h}h ${m}m` : `${h}h`) : `${m}m`;
}
function fmtClock(sec) {
  sec = Math.max(0, Math.floor(sec));
  return `${pad(Math.floor(sec / 3600))}:${pad(Math.floor((sec % 3600) / 60))}:${pad(sec % 60)}`;
}
const fmtDateTime = (iso) => (iso ? new Date(iso).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }) : "—");
const fmtDate = (d) => new Date(d).toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" });
const dateInputValue = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
function dayISO(value, addDays = 0) {
  const [y, m, d] = value.split("-").map(Number);
  return new Date(y, m - 1, d + addDays).toISOString();
}
function debounce(fn, ms = 250) {
  let t;
  return (...args) => { clearTimeout(t); t = setTimeout(() => fn(...args), ms); };
}

// Mirrors the backend's compute_bill(); used for live estimates only.
function priceFor(plan, usedMinutes, taxPercent) {
  const step = Math.max(1, Number(plan.increment_minutes) || 1);
  const billable = Math.max(Math.ceil(usedMinutes / step) * step, Number(plan.minimum_minutes) || 0);
  let subtotal = round2((billable * Number(plan.rate_per_hour)) / 60);
  if (plan.max_charge > 0) subtotal = Math.min(subtotal, round2(plan.max_charge));
  const tax = round2((subtotal * taxPercent) / 100);
  return { billable, subtotal, tax, total: round2(subtotal + tax) };
}
function estimate(session, nowMs) {
  const sec = Math.max(0, (nowMs - Date.parse(session.check_in)) / 1000);
  return { sec, ...priceFor(session, Math.ceil(Math.floor(sec) / 60), session.tax_percent) };
}
function billingRule(p) {
  const parts = [];
  if (p.minimum_minutes > 0) parts.push(`min ${fmtMinutes(p.minimum_minutes)}`);
  if (p.increment_minutes > 1) parts.push(`rounded up to ${p.increment_minutes} min`);
  if (p.max_charge > 0) parts.push(`max ${money(p.max_charge)}`);
  return parts.join(" · ");
}

async function api(path, { method = "GET", body } = {}) {
  try {
    return await window.__TAURI__.core.invoke("api", { method, path, body: body ?? null });
  } catch (e) {
    const err = e && typeof e === "object" ? e : { status: 500, error: String(e) };
    if (err.status === 401 && !path.startsWith("/api/auth/")) {
      showAuth("Your session has ended. Please sign in again.");
      throw new Error("Please sign in.");
    }
    throw new Error(err.error || "Something went wrong.");
  }
}

async function saveFile(path, what) {
  await guard(async () => {
    const res = await api(path, { method: "POST" });
    if (res.saved) toast(`${what} saved to ${res.saved}`);
  });
}

function toast(message, kind = "success") {
  const el = document.createElement("div");
  el.className = `toast ${kind}`;
  el.textContent = message;
  $("#toasts").appendChild(el);
  setTimeout(() => el.classList.add("hide"), 3500);
  setTimeout(() => el.remove(), 4000);
}

async function guard(fn) {
  try { return await fn(); } catch (e) { toast(e.message, "error"); }
}

function formData(form) {
  const data = Object.fromEntries(new FormData(form).entries());
  $$("input[type=checkbox]", form).forEach((c) => { data[c.name] = c.checked; });
  return data;
}

function field(name, label, value = "", { type = "text", required = false, attrs = "", cls = "" } = {}) {
  return `<div class="field ${cls}">
    <label for="f_${name}">${esc(label)}${required ? ' <span class="req">*</span>' : ""}</label>
    <input id="f_${name}" name="${name}" type="${type}" value="${esc(value ?? "")}" ${required ? "required" : ""} ${attrs}>
  </div>`;
}

// ------------------------------------------------------------ modal

let onModalClose = null;

function openModal({ title, body, footer = "", wide = false, onClose = null }) {
  closeModal();
  const root = $("#modal-root");
  root.innerHTML = `
    <div class="overlay">
      <div class="modal ${wide ? "wide" : ""}" role="dialog" aria-modal="true" aria-label="${esc(title)}">
        <header><h3>${esc(title)}</h3><button class="icon-btn" data-close aria-label="Close">×</button></header>
        <div class="modal-body">${body}</div>
        ${footer ? `<footer>${footer}</footer>` : ""}
      </div>
    </div>`;
  const modal = $(".modal", root);
  $$("[data-close]", root).forEach((b) => b.addEventListener("click", closeModal));
  $(".overlay", root).addEventListener("mousedown", (e) => { if (e.target.classList.contains("overlay")) closeModal(); });
  const first = $$("input:not([type=hidden]), select, textarea", modal).find((el) => el.offsetParent !== null);
  if (first) first.focus();
  onModalClose = onClose;
  return modal;
}
function closeModal() {
  $("#modal-root").innerHTML = "";
  const callback = onModalClose;
  onModalClose = null;
  if (callback) callback();
}

// In-app replacement for window.confirm, which desktop webviews don't reliably support.
function confirmDialog(message, { confirmText = "Confirm", danger = false } = {}) {
  return new Promise((resolve) => {
    const modal = openModal({
      title: "Please confirm",
      body: `<p class="confirm-text">${esc(message)}</p>`,
      footer: `<button class="btn" data-close>Cancel</button><button class="btn ${danger ? "danger" : "primary"}" id="confirmOk">${esc(confirmText)}</button>`,
      onClose: () => resolve(false),
    });
    $("#confirmOk", modal).onclick = () => { resolve(true); closeModal(); };
    $("#confirmOk", modal).focus();
  });
}
function showFormError(modal, message) {
  const el = $(".form-error", modal);
  if (el) { el.textContent = message; el.classList.remove("info"); el.hidden = false; } else toast(message, "error");
}
function hideFormError(modal) { const el = $(".form-error", modal); if (el) el.hidden = true; }

document.addEventListener("keydown", (e) => { if (e.key === "Escape") closeModal(); });

// ------------------------------------------------------------ shared data

async function loadLookups() {
  const [settings, rates, computers] = await Promise.all([api("/api/settings"), api("/api/rates"), api("/api/computers")]);
  state.settings = settings;
  state.rates = rates.items;
  state.computers = computers.items;
  $("#brandName").textContent = settings.library_name;
  document.title = settings.library_name;
}

// ------------------------------------------------------------ dashboard

async function loadDashboard() {
  const [stats, sessions] = await Promise.all([api("/api/dashboard"), api("/api/sessions?status=active"), loadLookups()]);
  state.clockOffset = Date.parse(sessions.server_now) - Date.now();
  state.active = sessions.items;
  const cards = [
    ["Checked in now", stats.active_sessions, ""],
    ["Computers free", `${stats.computers_free} / ${stats.computers_total}`, ""],
    ["Today's revenue", money(stats.today_revenue), "accent"],
    ["Today's check-ins", stats.today_checkins, ""],
    ["Usage today", fmtMinutes(stats.today_minutes), ""],
    ["Members", stats.members, ""],
  ];
  $("#stats").innerHTML = cards.map(([label, value, cls]) => `<div class="stat ${cls}"><div class="stat-label">${esc(label)}</div><div class="stat-value">${esc(value)}</div></div>`).join("");
  renderActive();
}

function renderActive() {
  $("#activeCount").textContent = state.active.length;
  const body = $("#activeBody");
  if (!state.active.length) {
    body.innerHTML = `<tr><td colspan="7" class="empty">No one is checked in right now. Click “New check-in” to start a session.</td></tr>`;
    return;
  }
  body.innerHTML = state.active.map((s) => `
    <tr data-id="${s.id}">
      <td><strong>${esc(fullName(s))}</strong><div class="muted small">ID ${esc(s.id_number)} · ${esc(s.phone)}</div></td>
      <td>${esc(s.computer_name || "—")}</td>
      <td>${esc(s.plan_name)}<div class="muted small">${money(s.rate_per_hour)}/hr</div></td>
      <td>${fmtDateTime(s.check_in)}</td>
      <td class="mono" data-elapsed></td>
      <td class="mono num strong" data-amount></td>
      <td class="actions">
        <button class="btn sm primary" data-act="checkout" data-id="${s.id}">Check out</button>
        <button class="btn sm ghost" data-act="cancel" data-id="${s.id}" title="Cancel without charging">Cancel</button>
      </td>
    </tr>`).join("");
  tickActive();
}

function tickActive() {
  if ($("#view-dashboard").hidden) return;
  const now = Date.now() + state.clockOffset;
  for (const s of state.active) {
    const row = $(`#activeBody tr[data-id="${s.id}"]`);
    if (!row) continue;
    const e = estimate(s, now);
    $("[data-elapsed]", row).textContent = fmtClock(e.sec);
    $("[data-amount]", row).textContent = money(e.total);
  }
}

async function cancelSession(id) {
  if (!(await confirmDialog("Cancel this session without charging? This cannot be undone.", { confirmText: "Cancel session", danger: true }))) return;
  await guard(async () => {
    await api(`/api/sessions/${id}/cancel`, { method: "POST" });
    toast("Session cancelled.");
    refreshCurrent();
  });
}

// ------------------------------------------------------------ check-in

async function openCheckin({ member = null, computerId = null } = {}) {
  try { await loadLookups(); } catch (e) { return toast(e.message, "error"); }
  const free = state.computers.filter((c) => c.active && !c.session_id);
  const plans = state.rates.filter((r) => r.active);
  if (!plans.length) return toast("Add an active rate plan in the Rate Card first.", "error");
  let selected = member;

  const computerOptions = free.map((c) => `<option value="${c.id}" ${c.id === computerId ? "selected" : ""}>${esc(c.name)}${c.location ? ` · ${esc(c.location)}` : ""}</option>`).join("");
  const modal = openModal({
    title: "New check-in",
    body: `
      <div class="field">
        <label>Member <span class="req">*</span></label>
        <div id="ciPicked"></div>
        <div id="ciPicker">
          <input type="search" id="ciSearch" placeholder="Search by name, ID number, phone or email" autocomplete="off">
          <div class="picker-results" id="ciResults"></div>
          <button type="button" class="link-btn" id="ciNew">+ Register a new member</button>
        </div>
      </div>
      <div class="form-grid">
        <div class="field">
          <label for="ciComputer">Computer</label>
          <select id="ciComputer">${computerOptions}<option value="">No computer assigned</option></select>
          ${free.length ? "" : '<small class="muted">All computers are currently in use.</small>'}
        </div>
        <div class="field">
          <label for="ciPlan">Rate plan</label>
          <select id="ciPlan">${plans.map((p) => `<option value="${p.id}" ${p.is_default ? "selected" : ""}>${esc(p.name)} — ${money(p.rate_per_hour)}/hr</option>`).join("")}</select>
          <small class="muted" id="ciPlanRule"></small>
        </div>
      </div>
      <div class="field"><label for="ciNotes">Notes</label><input id="ciNotes" maxlength="500" placeholder="Optional"></div>
      <p class="form-error" hidden></p>`,
    footer: `<button class="btn" data-close>Cancel</button><button class="btn primary" id="ciSubmit">Check in</button>`,
  });

  const renderPicked = () => {
    $("#ciPicked", modal).innerHTML = selected ? `
      <div class="picked">
        <div><strong>${esc(fullName(selected))}</strong><div class="muted small">ID ${esc(selected.id_number)} · ${esc(selected.phone)} · ${esc(selected.email)}</div></div>
        <button type="button" class="btn sm" id="ciChange">Change</button>
      </div>` : "";
    $("#ciPicker", modal).hidden = !!selected;
    if (selected) $("#ciChange", modal).onclick = () => { selected = null; renderPicked(); $("#ciSearch", modal).focus(); };
  };
  const renderRule = () => {
    const plan = plans.find((p) => String(p.id) === $("#ciPlan", modal).value);
    $("#ciPlanRule", modal).textContent = plan ? billingRule(plan) : "";
  };

  const search = debounce(async () => {
    const q = $("#ciSearch", modal).value.trim();
    const box = $("#ciResults", modal);
    if (!q) { box.innerHTML = ""; return; }
    try {
      const { items } = await api(`/api/members?q=${encodeURIComponent(q)}&limit=8`);
      box.innerHTML = items.length
        ? items.map((m) => `
          <button type="button" class="picker-item" data-id="${m.id}" ${m.active_session_id ? "disabled" : ""}>
            <span><strong>${esc(fullName(m))}</strong> <span class="muted small">ID ${esc(m.id_number)}</span></span>
            <span class="muted small">${m.active_session_id ? "Already checked in" : esc(m.phone)}</span>
          </button>`).join("")
        : `<div class="muted small pad-sm">No members found. Register them below.</div>`;
      $$(".picker-item", box).forEach((btn) => {
        btn.onclick = () => { selected = items.find((i) => String(i.id) === btn.dataset.id); renderPicked(); };
      });
    } catch (e) {
      box.innerHTML = `<div class="form-error">${esc(e.message)}</div>`;
    }
  }, 200);

  $("#ciSearch", modal).addEventListener("input", search);
  $("#ciPlan", modal).addEventListener("change", renderRule);
  $("#ciNew", modal).onclick = () => {
    const chosen = Number($("#ciComputer", modal).value) || null;
    openMemberForm(null, (m) => openCheckin({ member: m, computerId: chosen }));
  };
  $("#ciSubmit", modal).onclick = async () => {
    if (!selected) return showFormError(modal, "Select a member to check in.");
    try {
      await api("/api/sessions/checkin", {
        method: "POST",
        body: {
          member_id: selected.id,
          computer_id: $("#ciComputer", modal).value || null,
          rate_plan_id: $("#ciPlan", modal).value,
          notes: $("#ciNotes", modal).value,
        },
      });
      toast(`${fullName(selected)} checked in.`);
      closeModal();
      refreshCurrent();
    } catch (e) {
      showFormError(modal, e.message);
    }
  };
  renderPicked();
  renderRule();
  if (!selected) $("#ciSearch", modal).focus();
}

// ------------------------------------------------------------ check-out & receipts

function billTable(s, b) {
  const rule = billingRule(s);
  return `<table class="bill">
    <tr><td>Checked in</td><td>${fmtDateTime(s.check_in)}</td></tr>
    <tr><td>Checked out</td><td>${fmtDateTime(b.check_out)}</td></tr>
    <tr><td>Time used</td><td>${fmtMinutes(b.used_minutes)}${b.elapsed_seconds != null ? ` <span class="muted small">(${fmtClock(b.elapsed_seconds)})</span>` : ""}</td></tr>
    <tr><td>Billable time</td><td>${fmtMinutes(b.billable_minutes)}${rule ? `<div class="muted small">${esc(rule)}</div>` : ""}</td></tr>
    <tr><td>Rate</td><td>${esc(s.plan_name)} · ${money(s.rate_per_hour)}/hr</td></tr>
    <tr><td>Subtotal</td><td>${money(b.subtotal)}</td></tr>
    ${b.discount > 0 ? `<tr><td>Discount</td><td>− ${money(b.discount)}</td></tr>` : ""}
    ${s.tax_percent > 0 ? `<tr><td>Tax (${esc(s.tax_percent)}%)</td><td>${money(b.tax)}</td></tr>` : ""}
    <tr class="total"><td>Total</td><td>${money(b.total)}</td></tr>
  </table>`;
}

async function openCheckout(id) {
  let quote;
  try {
    if (!state.settings.payment_methods || state.settings.payment_methods.length < 2) await loadLookups();
    quote = await api(`/api/sessions/${id}/quote`);
  } catch (e) { return toast(e.message, "error"); }
  const s = quote.session;
  const modal = openModal({
    title: "Check out",
    body: `
      <div class="checkout-head">
        <div><div class="muted small">Member</div><strong>${esc(fullName(s))}</strong><div class="muted small">ID ${esc(s.id_number)} · ${esc(s.phone)}</div></div>
        <div><div class="muted small">Computer</div><strong>${esc(s.computer_name || "—")}</strong></div>
      </div>
      <div id="coBill"></div>
      <div class="form-grid">
        <div class="field"><label for="coDiscount">Discount (${esc(state.settings.currency)})</label><input id="coDiscount" type="number" min="0" step="0.01" value="0"></div>
        <div class="field"><label for="coPayment">Payment method</label>
          <select id="coPayment">${state.settings.payment_methods.map((p) => `<option>${esc(p)}</option>`).join("")}</select></div>
      </div>
      <div class="field"><label for="coNotes">Notes</label><input id="coNotes" maxlength="500" value="${esc(s.notes || "")}"></div>
      <p class="form-error" hidden></p>`,
    footer: `<button class="btn" data-close>Back</button><button class="btn primary" id="coSubmit">Confirm &amp; collect <span id="coTotal"></span></button>`,
  });
  const render = (b) => { $("#coBill", modal).innerHTML = billTable(s, b); $("#coTotal", modal).textContent = money(b.total); };
  render(quote.bill);

  const requote = debounce(async () => {
    try {
      const q = await api(`/api/sessions/${id}/quote?discount=${encodeURIComponent($("#coDiscount", modal).value || 0)}`);
      render(q.bill);
      hideFormError(modal);
    } catch (e) { showFormError(modal, e.message); }
  }, 250);
  $("#coDiscount", modal).addEventListener("input", requote);

  $("#coSubmit", modal).onclick = async (ev) => {
    ev.currentTarget.disabled = true;
    try {
      const done = await api(`/api/sessions/${id}/checkout`, {
        method: "POST",
        body: { discount: $("#coDiscount", modal).value || 0, payment_method: $("#coPayment", modal).value, notes: $("#coNotes", modal).value },
      });
      toast(`${fullName(done)} checked out · ${money(done.total)}`);
      refreshCurrent();
      showReceipt(done);
    } catch (e) {
      ev.currentTarget.disabled = false;
      showFormError(modal, e.message);
    }
  };
}

function showReceipt(s) {
  const b = {
    check_out: s.check_out, used_minutes: s.duration_minutes, billable_minutes: s.billable_minutes,
    subtotal: s.subtotal, discount: s.discount, tax: s.tax, total: s.total,
  };
  const modal = openModal({
    title: `Receipt #${String(s.id).padStart(6, "0")}`,
    body: `
      <div class="receipt" id="receipt">
        <div class="receipt-head">
          <h2>${esc(state.settings.library_name)}</h2>
          <div class="muted">Computer usage receipt</div>
          <div class="muted small">Receipt #${String(s.id).padStart(6, "0")} · ${fmtDateTime(s.check_out)}</div>
        </div>
        <table class="bill">
          <tr><td>Member</td><td>${esc(fullName(s))}</td></tr>
          <tr><td>ID number</td><td>${esc(s.id_number)}</td></tr>
          <tr><td>Phone</td><td>${esc(s.phone)}</td></tr>
          <tr><td>Email</td><td>${esc(s.email)}</td></tr>
          <tr><td>Computer</td><td>${esc(s.computer_name || "—")}</td></tr>
        </table>
        ${billTable(s, b)}
        <table class="bill">
          <tr><td>Status</td><td>${esc(s.status)}</td></tr>
          ${s.payment_method ? `<tr><td>Paid by</td><td>${esc(s.payment_method)}</td></tr>` : ""}
          ${s.notes ? `<tr><td>Notes</td><td>${esc(s.notes)}</td></tr>` : ""}
        </table>
        <p class="thanks">Thank you for visiting!</p>
      </div>`,
    footer: `<button class="btn" data-close>Close</button><button class="btn primary" id="btnPrint">Print receipt</button>`,
  });
  $("#btnPrint", modal).onclick = () => guard(() => api("/api/print", { method: "POST" }));
}

async function openReceipt(id) {
  await guard(async () => showReceipt(await api(`/api/sessions/${id}`)));
}

// ------------------------------------------------------------ members

async function loadMembers() {
  const q = $("#memberSearch").value.trim();
  const { items } = await api(`/api/members?q=${encodeURIComponent(q)}`);
  state.members = items;
  $("#membersBody").innerHTML = items.length ? items.map((m) => `
    <tr>
      <td><strong>${esc(fullName(m))}</strong>${m.notes ? `<div class="muted small">${esc(m.notes)}</div>` : ""}</td>
      <td class="mono">${esc(m.id_number)}</td>
      <td>${esc(m.email)}</td>
      <td>${esc(m.phone)}</td>
      <td class="num">${m.visits}</td>
      <td class="num">${money(m.total_spent)}</td>
      <td>${m.active_session_id ? '<span class="badge live">Checked in</span>' : '<span class="badge">Away</span>'}</td>
      <td class="actions">
        ${m.active_session_id
          ? `<button class="btn sm primary" data-act="checkout" data-id="${m.active_session_id}">Check out</button>`
          : `<button class="btn sm primary" data-act="checkin" data-id="${m.id}">Check in</button>`}
        <button class="btn sm" data-act="edit" data-id="${m.id}">Edit</button>
        <button class="btn sm" data-act="history" data-id="${m.id}">History</button>
        <button class="btn sm ghost danger-text" data-act="delete" data-id="${m.id}">Delete</button>
      </td>
    </tr>`).join("")
    : `<tr><td colspan="8" class="empty">${q ? "No members match your search." : "No members yet. Click “Add member” to register one."}</td></tr>`;
}

function openMemberForm(member = null, onSaved = null) {
  const m = member || {};
  const modal = openModal({
    title: member ? "Edit member" : "Register new member",
    body: `
      <form id="memberForm" class="form-grid" novalidate>
        ${field("first_name", "First name", m.first_name, { required: true, attrs: 'maxlength="80" autocomplete="off"' })}
        ${field("last_name", "Last name", m.last_name, { required: true, attrs: 'maxlength="80" autocomplete="off"' })}
        ${field("email", "Email", m.email, { type: "email", required: true, attrs: 'maxlength="160" autocomplete="off"' })}
        ${field("phone", "Phone", m.phone, { type: "tel", required: true, attrs: 'maxlength="25" autocomplete="off"' })}
        ${field("id_number", "ID number", m.id_number, { required: true, cls: "span2", attrs: 'maxlength="60" autocomplete="off" placeholder="Library card / Aadhaar / Student ID etc."' })}
        <div class="field span2"><label for="f_notes">Notes</label><textarea id="f_notes" name="notes" rows="2" maxlength="1000">${esc(m.notes || "")}</textarea></div>
        <p class="form-error span2" hidden></p>
      </form>`,
    footer: `<button class="btn" data-close>Cancel</button><button class="btn primary" type="submit" form="memberForm">${member ? "Save changes" : "Register member"}</button>`,
  });
  $("#memberForm", modal).addEventListener("submit", async (e) => {
    e.preventDefault();
    try {
      const data = formData(e.target);
      const saved = member
        ? await api(`/api/members/${member.id}`, { method: "PUT", body: data })
        : await api("/api/members", { method: "POST", body: data });
      toast(member ? "Member updated." : `${fullName(saved)} registered.`);
      closeModal();
      if (onSaved) onSaved(saved); else refreshCurrent();
    } catch (err) {
      showFormError(modal, err.message);
    }
  });
}

async function openMemberHistory(member) {
  await guard(async () => {
    const { items } = await api(`/api/sessions?member_id=${member.id}`);
    const spent = items.filter((s) => s.status === "completed").reduce((a, s) => a + s.total, 0);
    const modal = openModal({
      title: `History · ${fullName(member)}`,
      wide: true,
      body: `
        <p class="muted small">ID ${esc(member.id_number)} · ${esc(member.email)} · ${esc(member.phone)} · ${items.length} session(s) · Total spent ${money(spent)}</p>
        <div class="table-wrap"><table>
          <thead><tr><th>Receipt</th><th>Computer</th><th>Plan</th><th>Check in</th><th>Duration</th><th class="num">Total</th><th>Status</th><th></th></tr></thead>
          <tbody>${items.length ? items.map((s) => `
            <tr>
              <td class="mono">#${String(s.id).padStart(6, "0")}</td>
              <td>${esc(s.computer_name || "—")}</td>
              <td>${esc(s.plan_name)}</td>
              <td>${fmtDateTime(s.check_in)}</td>
              <td>${s.duration_minutes != null ? fmtMinutes(s.duration_minutes) : "—"}</td>
              <td class="num">${s.total != null ? money(s.total) : "—"}</td>
              <td>${statusBadge(s.status)}</td>
              <td>${s.status !== "active" ? `<button class="btn sm" data-receipt="${s.id}">Receipt</button>` : ""}</td>
            </tr>`).join("") : `<tr><td colspan="8" class="empty">No sessions yet.</td></tr>`}</tbody>
        </table></div>`,
      footer: `<button class="btn" data-close>Close</button>`,
    });
    $$("[data-receipt]", modal).forEach((b) => (b.onclick = () => openReceipt(b.dataset.receipt)));
  });
}

async function deleteMember(member) {
  if (!(await confirmDialog(`Delete ${fullName(member)}? This cannot be undone.`, { confirmText: "Delete", danger: true }))) return;
  await guard(async () => {
    await api(`/api/members/${member.id}`, { method: "DELETE" });
    toast("Member deleted.");
    loadMembers();
  });
}

// ------------------------------------------------------------ history

const statusBadge = (s) => `<span class="badge ${s === "active" ? "live" : s === "cancelled" ? "muted-badge" : "ok"}">${esc(s)}</span>`;

function historyParams() {
  const p = new URLSearchParams();
  if ($("#hFrom").value) p.set("from", dayISO($("#hFrom").value));
  if ($("#hTo").value) p.set("to", dayISO($("#hTo").value, 1));
  if ($("#hStatus").value) p.set("status", $("#hStatus").value);
  if ($("#hSearch").value.trim()) p.set("q", $("#hSearch").value.trim());
  return p;
}

async function loadHistory() {
  const { items } = await api(`/api/sessions?${historyParams()}`);
  state.history = items;
  $("#historyBody").innerHTML = items.length ? items.map((s) => `
    <tr>
      <td class="mono">#${String(s.id).padStart(6, "0")}</td>
      <td><strong>${esc(fullName(s))}</strong><div class="muted small">ID ${esc(s.id_number)}</div></td>
      <td>${esc(s.computer_name || "—")}</td>
      <td>${esc(s.plan_name)}</td>
      <td>${fmtDateTime(s.check_in)}</td>
      <td>${fmtDateTime(s.check_out)}</td>
      <td>${s.duration_minutes != null ? fmtMinutes(s.duration_minutes) : "—"}</td>
      <td class="num">${s.total != null ? money(s.total) : "—"}</td>
      <td>${esc(s.payment_method || "—")}</td>
      <td>${statusBadge(s.status)}</td>
      <td class="actions">${s.status === "active"
        ? `<button class="btn sm primary" data-act="checkout" data-id="${s.id}">Check out</button>`
        : `<button class="btn sm" data-act="receipt" data-id="${s.id}">Receipt</button>`}</td>
    </tr>`).join("") : `<tr><td colspan="11" class="empty">No sessions found for these filters.</td></tr>`;
  const done = items.filter((s) => s.status === "completed");
  const revenue = done.reduce((a, s) => a + s.total, 0);
  const minutes = done.reduce((a, s) => a + s.duration_minutes, 0);
  $("#historySummary").textContent = `${items.length} session(s) shown · ${done.length} completed · ${fmtMinutes(minutes)} used · ${money(revenue)} collected`;
}

// ------------------------------------------------------------ reports

function setReportPreset(preset) {
  const today = new Date();
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  if (preset === "7d") start.setDate(start.getDate() - 6);
  if (preset === "30d") start.setDate(start.getDate() - 29);
  if (preset === "month") start.setDate(1);
  $("#rFrom").value = dateInputValue(start);
  $("#rTo").value = dateInputValue(today);
  $$("#rPresets button").forEach((b) => b.classList.toggle("active", b.dataset.preset === preset));
  loadReports();
}

function reportTable(rowsData, columns, emptyText = "No data for this period.") {
  if (!rowsData.length) return `<p class="empty">${emptyText}</p>`;
  const max = Math.max(...rowsData.map((r) => r.revenue), 1);
  return `<div class="table-wrap"><table>
    <thead><tr>${columns.map((c) => `<th class="${c.num ? "num" : ""}">${c.label}</th>`).join("")}<th></th></tr></thead>
    <tbody>${rowsData.map((r) => `<tr>${columns.map((c) => `<td class="${c.num ? "num" : ""}">${c.fmt(r)}</td>`).join("")}
      <td class="bar-cell"><div class="bar" style="width:${Math.round((r.revenue / max) * 100)}%"></div></td></tr>`).join("")}</tbody>
  </table></div>`;
}

async function loadReports() {
  if (!$("#rFrom").value || !$("#rTo").value) return setReportPreset("7d");
  const data = await api(`/api/reports?from=${encodeURIComponent(dayISO($("#rFrom").value))}&to=${encodeURIComponent(dayISO($("#rTo").value, 1))}`);
  const s = data.summary;
  const cards = [
    ["Revenue", money(s.revenue), "accent"],
    ["Sessions", s.sessions, ""],
    ["Time used", fmtMinutes(s.minutes), ""],
    ["Avg session", fmtMinutes(s.avg_minutes), ""],
    ["Avg bill", money(s.avg_revenue), ""],
    ["Discounts", money(s.discount), ""],
    ["Tax collected", money(s.tax), ""],
  ];
  $("#reportStats").innerHTML = cards.map(([l, v, c]) => `<div class="stat ${c}"><div class="stat-label">${l}</div><div class="stat-value">${esc(v)}</div></div>`).join("");
  const common = [
    { label: "Sessions", num: true, fmt: (r) => r.sessions },
    { label: "Time", num: true, fmt: (r) => fmtMinutes(r.minutes) },
    { label: "Revenue", num: true, fmt: (r) => money(r.revenue) },
  ];
  $("#rByDay").innerHTML = reportTable(data.by_day, [{ label: "Day", fmt: (r) => esc(fmtDate(r.name + "T00:00:00")) }, ...common]);
  $("#rByPayment").innerHTML = reportTable(data.by_payment, [{ label: "Method", fmt: (r) => esc(r.name) }, ...common]);
  $("#rByPlan").innerHTML = reportTable(data.by_plan, [{ label: "Plan", fmt: (r) => esc(r.name) }, ...common]);
  $("#rTopMembers").innerHTML = reportTable(data.top_members, [{ label: "Member", fmt: (r) => esc(r.name) }, ...common]);
}

// ------------------------------------------------------------ rate card

async function loadRates() {
  await loadLookups();
  $("#taxPercent").value = state.settings.tax_percent;
  $("#currency").value = state.settings.currency;
  $("#ratesBody").innerHTML = state.rates.length ? state.rates.map((r) => `
    <tr class="${r.active ? "" : "dim"}">
      <td><strong>${esc(r.name)}</strong> ${r.is_default ? '<span class="badge ok">Default</span>' : ""}${r.description ? `<div class="muted small">${esc(r.description)}</div>` : ""}</td>
      <td class="num strong">${money(r.rate_per_hour)}</td>
      <td class="num">${r.minimum_minutes ? fmtMinutes(r.minimum_minutes) : "—"}</td>
      <td class="num">${r.increment_minutes} min</td>
      <td class="num">${r.max_charge > 0 ? money(r.max_charge) : "No cap"}</td>
      <td>${r.active ? '<span class="badge ok">Active</span>' : '<span class="badge muted-badge">Inactive</span>'}</td>
      <td class="actions">
        <button class="btn sm" data-act="edit-rate" data-id="${r.id}">Edit</button>
        <button class="btn sm ghost danger-text" data-act="delete-rate" data-id="${r.id}">Delete</button>
      </td>
    </tr>`).join("") : `<tr><td colspan="7" class="empty">No rate plans yet.</td></tr>`;
  renderCalculator();
}

function renderCalculator() {
  const minutes = Math.max(0, Number($("#calcMinutes").value) || 0);
  const plans = state.rates.filter((r) => r.active);
  $("#calcResult").innerHTML = plans.length ? `<table class="bill">${plans.map((p) => {
    const price = priceFor(p, minutes, state.settings.tax_percent);
    return `<tr><td>${esc(p.name)} <span class="muted small">bills ${fmtMinutes(price.billable)}</span></td><td>${money(price.total)}</td></tr>`;
  }).join("")}</table>` : `<p class="muted small">No active plans.</p>`;
}

function openRateForm(rate = null) {
  const r = rate || { minimum_minutes: 30, increment_minutes: 15, max_charge: 0, active: 1, is_default: 0 };
  const modal = openModal({
    title: rate ? `Edit rate plan · ${rate.name}` : "New rate plan",
    body: `
      <form id="rateForm" class="form-grid" novalidate>
        ${field("name", "Plan name", r.name, { required: true, attrs: 'maxlength="60"' })}
        ${field("rate_per_hour", `Rate per hour (${state.settings.currency})`, r.rate_per_hour, { type: "number", required: true, attrs: 'min="0" step="0.01"' })}
        ${field("minimum_minutes", "Minimum charge (minutes)", r.minimum_minutes, { type: "number", attrs: 'min="0" max="1440" step="1"' })}
        ${field("increment_minutes", "Round up to (minutes)", r.increment_minutes, { type: "number", attrs: 'min="1" max="1440" step="1"' })}
        ${field("max_charge", `Max charge per session (${state.settings.currency}, 0 = no cap)`, r.max_charge, { type: "number", attrs: 'min="0" step="0.01"' })}
        ${field("description", "Description", r.description, { attrs: 'maxlength="200"' })}
        <label class="check"><input type="checkbox" name="active" ${r.active ? "checked" : ""}> Active</label>
        <label class="check"><input type="checkbox" name="is_default" ${r.is_default ? "checked" : ""}> Default plan for check-in</label>
        <div class="span2 example muted small" id="rateExample"></div>
        <p class="form-error span2" hidden></p>
      </form>`,
    footer: `<button class="btn" data-close>Cancel</button><button class="btn primary" type="submit" form="rateForm">${rate ? "Save changes" : "Create plan"}</button>`,
  });
  const form = $("#rateForm", modal);
  const example = () => {
    const d = formData(form);
    const plan = { rate_per_hour: Number(d.rate_per_hour) || 0, minimum_minutes: Number(d.minimum_minutes) || 0, increment_minutes: Number(d.increment_minutes) || 1, max_charge: Number(d.max_charge) || 0 };
    $("#rateExample", modal).textContent = "Examples (before tax): " + [10, 47, 90, 180].map((m) => `${fmtMinutes(m)} → ${money(priceFor(plan, m, 0).total)}`).join(" · ");
  };
  form.addEventListener("input", example);
  example();
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    try {
      const data = formData(form);
      await (rate ? api(`/api/rates/${rate.id}`, { method: "PUT", body: data }) : api("/api/rates", { method: "POST", body: data }));
      toast(rate ? "Rate plan updated." : "Rate plan created.");
      closeModal();
      loadRates();
    } catch (err) { showFormError(modal, err.message); }
  });
}

async function deleteRate(rate) {
  if (!(await confirmDialog(`Delete rate plan "${rate.name}"? Past receipts keep their original price.`, { confirmText: "Delete", danger: true }))) return;
  await guard(async () => {
    await api(`/api/rates/${rate.id}`, { method: "DELETE" });
    toast("Rate plan deleted.");
    loadRates();
  });
}

// ------------------------------------------------------------ computers

async function loadComputers() {
  await loadLookups();
  const now = Date.now();
  $("#computerGrid").innerHTML = state.computers.length ? state.computers.map((c) => {
    const status = !c.active ? ["off", "Disabled"] : c.session_id ? ["busy", "In use"] : ["free", "Available"];
    return `<div class="pc ${status[0]}">
      <div class="pc-top"><span class="pc-name">${esc(c.name)}</span><span class="badge ${status[0]}">${status[1]}</span></div>
      <div class="muted small">${esc(c.location || "No location")}</div>
      <div class="pc-body">${c.session_id
        ? `<strong>${esc(c.session_member)}</strong><div class="muted small">since ${fmtDateTime(c.session_check_in)} · ${fmtMinutes((now - Date.parse(c.session_check_in)) / 60000)}</div>`
        : c.active ? `<button class="btn sm primary" data-act="checkin-pc" data-id="${c.id}">Check in here</button>` : ""}</div>
      <div class="pc-actions">
        ${c.session_id ? `<button class="btn sm primary" data-act="checkout" data-id="${c.session_id}">Check out</button>` : ""}
        <button class="btn sm" data-act="edit-pc" data-id="${c.id}">Edit</button>
        <button class="btn sm ghost danger-text" data-act="delete-pc" data-id="${c.id}">Delete</button>
      </div>
    </div>`;
  }).join("") : `<div class="card empty">No computers yet. Click “Add computer”.</div>`;
}

function openComputerForm(pc = null) {
  const c = pc || { active: 1 };
  const modal = openModal({
    title: pc ? `Edit ${pc.name}` : "Add computer",
    body: `
      <form id="pcForm" novalidate>
        ${field("name", "Name", c.name, { required: true, attrs: 'maxlength="60" placeholder="e.g. PC-07"' })}
        ${field("location", "Location", c.location, { attrs: 'maxlength="120" placeholder="e.g. Main hall"' })}
        <label class="check"><input type="checkbox" name="active" ${c.active ? "checked" : ""}> Available for check-in</label>
        <p class="form-error" hidden></p>
      </form>`,
    footer: `<button class="btn" data-close>Cancel</button><button class="btn primary" type="submit" form="pcForm">${pc ? "Save" : "Add computer"}</button>`,
  });
  $("#pcForm", modal).addEventListener("submit", async (e) => {
    e.preventDefault();
    try {
      const data = formData(e.target);
      await (pc ? api(`/api/computers/${pc.id}`, { method: "PUT", body: data }) : api("/api/computers", { method: "POST", body: data }));
      toast(pc ? "Computer updated." : "Computer added.");
      closeModal();
      loadComputers();
    } catch (err) { showFormError(modal, err.message); }
  });
}

async function deleteComputer(pc) {
  if (!(await confirmDialog(`Delete ${pc.name}? Past sessions keep its name.`, { confirmText: "Delete", danger: true }))) return;
  await guard(async () => {
    await api(`/api/computers/${pc.id}`, { method: "DELETE" });
    toast("Computer deleted.");
    loadComputers();
  });
}

// ------------------------------------------------------------ settings

async function loadSettings() {
  await loadLookups();
  $("#sLibraryName").value = state.settings.library_name;
  $("#sDataDir").textContent = state.settings.data_dir;
  $("#sVersion").textContent = `Version ${state.settings.version}`;
}

// ------------------------------------------------------------ authentication

function showAuthForm(which) {
  $("#loginForm").hidden = which !== "login";
  $("#resetForm").hidden = which !== "reset";
  $$("#authScreen .form-error").forEach((el) => { el.hidden = true; });
  const focus = which === "reset" ? "#resetUser" : $("#loginUser").value ? "#loginPass" : "#loginUser";
  $(focus).focus();
}

function showAuth(message = "", info = false) {
  state.auth = { ...state.auth, authenticated: false };
  closeModal();
  $("#app").hidden = true;
  $("#authScreen").hidden = false;
  $("#loginPass").value = "";
  showAuthForm("login");
  if (message) {
    const el = $("#loginForm .form-error");
    el.textContent = message;
    el.classList.toggle("info", info);
    el.hidden = false;
  }
}

function applyAuth(status) {
  state.auth = status;
  $("#authLibraryName").textContent = status.library_name || "Digital Library";
  if (!status.authenticated) return showAuth();
  $("#authScreen").hidden = true;
  $("#app").hidden = false;
  $("#whoami").textContent = status.username;
  $("#sUsername").textContent = status.username;
  $("#defaultPwBanner").hidden = !status.default_password;
  route();
}

async function checkAuth() {
  try { applyAuth(await api("/api/auth/status")); } catch (e) { showAuth(e.message); }
}

function bindAuthForms() {
  $("#loginForm").addEventListener("submit", async (e) => {
    e.preventDefault();
    const form = e.target, btn = $("button[type=submit]", form);
    btn.disabled = true;
    try {
      applyAuth(await api("/api/auth/login", { method: "POST", body: formData(form) }));
      $("#loginPass").value = "";
    } catch (err) {
      showFormError(form, err.message);
      $("#loginPass").select();
    } finally {
      btn.disabled = false;
    }
  });
  $("#resetForm").addEventListener("submit", async (e) => {
    e.preventDefault();
    const form = e.target, data = formData(form);
    if (data.new_password !== data.confirm_password) return showFormError(form, "The new passwords don't match.");
    try {
      await api("/api/auth/change-password", { method: "POST", body: data });
      form.reset();
      $("#loginUser").value = data.username;
      showAuth("Password reset. Sign in with your new password.", true);
    } catch (err) {
      showFormError(form, err.message);
    }
  });
  $("#showReset").onclick = () => { $("#resetUser").value = $("#loginUser").value; showAuthForm("reset"); };
  $("#showLogin").onclick = () => showAuthForm("login");

  $("#passwordForm").addEventListener("submit", async (e) => {
    e.preventDefault();
    const form = e.target, data = formData(form);
    hideFormError(form);
    if (data.new_password !== data.confirm_password) return showFormError(form, "The new passwords don't match.");
    try {
      await api("/api/auth/change-password", { method: "POST", body: data });
      form.reset();
      toast("Password changed.");
      state.auth = await api("/api/auth/status");
      $("#defaultPwBanner").hidden = !state.auth.default_password;
    } catch (err) {
      showFormError(form, err.message);
    }
  });
  $("#btnLogout").onclick = async () => {
    try { await api("/api/auth/logout", { method: "POST" }); } catch { /* signed out locally regardless */ }
    showAuth("You have been signed out.", true);
  };
}

// ------------------------------------------------------------ routing & events

const views = {
  dashboard: loadDashboard, members: loadMembers, history: loadHistory, reports: loadReports,
  rates: loadRates, computers: loadComputers, settings: loadSettings,
};

function currentView() {
  const name = location.hash.slice(1);
  return views[name] ? name : "dashboard";
}
function refreshCurrent() { guard(views[currentView()]); }
function route() {
  if (!state.auth.authenticated) return;
  const view = currentView();
  $$(".view").forEach((v) => { v.hidden = v.id !== `view-${view}`; });
  $$("nav a").forEach((a) => a.classList.toggle("active", a.dataset.view === view));
  refreshCurrent();
}

function bindActions(container, handlers) {
  container.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-act]");
    if (btn && handlers[btn.dataset.act]) handlers[btn.dataset.act](Number(btn.dataset.id));
  });
}

const findMember = (id) => state.members.find((m) => m.id === id);
const findRate = (id) => state.rates.find((r) => r.id === id);
const findPc = (id) => state.computers.find((c) => c.id === id);

function init() {
  $("#btnCheckin").onclick = () => openCheckin();
  $("#btnAddMember").onclick = () => openMemberForm();
  $("#btnAddRate").onclick = () => openRateForm();
  $("#btnAddComputer").onclick = () => openComputerForm();

  bindActions($("#activeBody"), { checkout: openCheckout, cancel: cancelSession });
  bindActions($("#membersBody"), {
    checkout: openCheckout,
    checkin: (id) => openCheckin({ member: findMember(id) }),
    edit: (id) => openMemberForm(findMember(id)),
    history: (id) => openMemberHistory(findMember(id)),
    delete: (id) => deleteMember(findMember(id)),
  });
  bindActions($("#historyBody"), { checkout: openCheckout, receipt: openReceipt });
  bindActions($("#ratesBody"), { "edit-rate": (id) => openRateForm(findRate(id)), "delete-rate": (id) => deleteRate(findRate(id)) });
  bindActions($("#computerGrid"), {
    checkout: openCheckout,
    "checkin-pc": (id) => openCheckin({ computerId: id }),
    "edit-pc": (id) => openComputerForm(findPc(id)),
    "delete-pc": (id) => deleteComputer(findPc(id)),
  });

  $("#memberSearch").addEventListener("input", debounce(() => guard(loadMembers)));
  ["#hFrom", "#hTo", "#hStatus"].forEach((s) => $(s).addEventListener("change", () => guard(loadHistory)));
  $("#hSearch").addEventListener("input", debounce(() => guard(loadHistory)));
  $("#hReset").onclick = () => { ["#hFrom", "#hTo", "#hStatus", "#hSearch"].forEach((s) => { $(s).value = ""; }); guard(loadHistory); };
  $("#btnExportSessions").onclick = () => saveFile(`/api/export/sessions?${historyParams()}`, "Sessions");
  $("#btnExportMembers").onclick = () => saveFile("/api/export/members", "Members");

  $$("#rPresets button").forEach((b) => (b.onclick = () => guard(() => setReportPreset(b.dataset.preset))));
  $("#rApply").onclick = () => { $$("#rPresets button").forEach((b) => b.classList.remove("active")); guard(loadReports); };

  $("#calcMinutes").addEventListener("input", renderCalculator);
  $("#billingForm").addEventListener("submit", (e) => {
    e.preventDefault();
    guard(async () => {
      await api("/api/settings", { method: "PUT", body: { tax_percent: $("#taxPercent").value || 0, currency: $("#currency").value } });
      toast("Billing settings saved. New check-ins will use them.");
      loadRates();
    });
  });
  $("#settingsForm").addEventListener("submit", (e) => {
    e.preventDefault();
    guard(async () => {
      await api("/api/settings", { method: "PUT", body: { library_name: $("#sLibraryName").value } });
      toast("Settings saved.");
      loadSettings();
    });
  });
  $("#btnBackup").onclick = () => saveFile("/api/backup", "Backup");
  $("#btnExportMembersAll").onclick = () => saveFile("/api/export/members", "Members");
  $("#btnExportSessionsAll").onclick = () => saveFile("/api/export/sessions", "Sessions");
  $("#btnRestore").onclick = async () => {
    const message = "Replace ALL current data, including the login password, with a backup file?\n\nA copy of the current data is saved first, so this can be undone.";
    if (!(await confirmDialog(message, { confirmText: "Choose backup file…", danger: true }))) return;
    await guard(async () => {
      const res = await api("/api/restore", { method: "POST" });
      if (res.cancelled) return;
      showAuth(`Restored ${res.members} member(s) and ${res.sessions} session(s). Sign in with the username and password from the backup.`, true);
    });
  };

  const tickClock = () => {
    const now = new Date();
    $("#clock").textContent = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    $("#date").textContent = now.toLocaleDateString([], { weekday: "long", day: "numeric", month: "long", year: "numeric" });
  };
  tickClock();
  setInterval(() => { tickClock(); tickActive(); }, 1000);
  setInterval(() => { if (state.auth.authenticated && currentView() === "dashboard" && !$("#modal-root").children.length) guard(loadDashboard); }, 30000);

  bindAuthForms();
  window.addEventListener("hashchange", route);
  checkAuth();
}

init();
