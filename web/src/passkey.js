"use strict";
// The dashboard's one script: WebAuthn is a browser API, so registering, signing in and removing a
// passkey call the JSON routes under /v1/passkeys. Served as a static file from this host.
const text = (buffer) => btoa(String.fromCharCode(...new Uint8Array(buffer))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const bytes = (value) => Uint8Array.from(atob(value.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(value.length / 4) * 4, "=")), (c) => c.charCodeAt(0));
const say = (message) => { const out = document.querySelector("[data-passkey-status]"); if (out) out.textContent = message; };

async function call(method, path, body) {
  const reply = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: body ? { "content-type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  const json = await reply.json().catch(() => ({}));
  if (!reply.ok) throw new Error((json.problem ? json.problem + ": " : "") + (json.message || reply.statusText));
  return json;
}

async function register() {
  const label = document.querySelector("[data-passkey-label]").value.trim() || "passkey";
  const options = await call("POST", "/v1/passkeys/options", { label });
  options.challenge = bytes(options.challenge);
  options.user.id = bytes(options.user.id);
  options.excludeCredentials = options.excludeCredentials.map((c) => ({ ...c, id: bytes(c.id) }));
  const made = await navigator.credentials.create({ publicKey: options });
  await call("POST", "/v1/passkeys", {
    label,
    credential: {
      id: text(made.rawId),
      clientData: text(made.response.clientDataJSON),
      attestation: text(made.response.attestationObject),
      transports: made.response.getTransports ? made.response.getTransports() : [],
    },
  });
  location.reload();
}

async function login() {
  const options = await call("POST", "/v1/passkeys/challenge");
  options.challenge = bytes(options.challenge);
  const got = await navigator.credentials.get({ publicKey: options });
  await call("POST", "/v1/passkeys/session", {
    credential: {
      id: text(got.rawId),
      clientData: text(got.response.clientDataJSON),
      authenticator: text(got.response.authenticatorData),
      signature: text(got.response.signature),
      handle: got.response.userHandle ? text(got.response.userHandle) : null,
    },
  });
  location.assign("/account");
}

async function remove(id) {
  await call("DELETE", "/v1/passkeys/" + encodeURIComponent(id));
  location.reload();
}

const actions = { register, login, remove: (button) => remove(button.dataset.passkeyId) };
document.addEventListener("click", (event) => {
  const button = event.target.closest("[data-passkey]");
  if (!button) return;
  event.preventDefault();
  button.disabled = true;
  say("Waiting for your authenticator...");
  actions[button.dataset.passkey](button)
    .catch((error) => say(error.message))
    .finally(() => { button.disabled = false; });
});
