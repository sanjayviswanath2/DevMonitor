import { useCallback, useEffect, useState } from "react";
import {
  clearGitIdentity,
  getGitStatus,
  installOrUpdateGit,
  removeGitHostAccount,
  setGitIdentity,
  storeGitCredentials,
  testGitHostConnection,
  uninstallGit,
  useGitHostAccount,
} from "../../lib/api";
import type { GitHostAccount, GitStatus } from "../../lib/types";

type Props = {
  projectPath: string | null;
  onToast: (kind: "success" | "error" | "info", message: string) => void;
};

type Host = "github" | "gitlab";
type ConfirmKind =
  | Host
  | "uninstall"
  | "clearIdentity"
  | "install"
  | "update"
  | "saveIdentity"
  | "overwriteCreds"
  | null;

function installChip(status: GitStatus): {
  label: string;
  kind: "success" | "warning" | "error" | "muted";
} {
  switch (status.installStatus) {
    case "installed":
      return { label: "Installed", kind: "success" };
    case "update_available":
      return { label: "Update available", kind: "warning" };
    case "not_installed":
      return { label: "Not installed", kind: "error" };
    case "error":
      return { label: "Error", kind: "error" };
    default:
      return { label: "Checking", kind: "muted" };
  }
}

function authChip(status: string): {
  label: string;
  kind: "success" | "warning" | "error" | "muted";
} {
  switch (status) {
    case "connected":
      return { label: "Connected", kind: "success" };
    case "unknown":
      return { label: "Status unknown", kind: "warning" };
    default:
      return { label: "Not connected", kind: "muted" };
  }
}

function isValidEmail(value: string): boolean {
  const v = value.trim();
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v);
}

export function GitDoctorView({ projectPath, onToast }: Props) {
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyGit, setBusyGit] = useState(false);
  const [signing, setSigning] = useState<Host | null>(null);
  const [removing, setRemoving] = useState<Host | null>(null);
  const [testing, setTesting] = useState<Host | null>(null);
  const [confirm, setConfirm] = useState<ConfirmKind>(null);
  const [editingIdentity, setEditingIdentity] = useState(false);
  const [savingManual, setSavingManual] = useState(false);
  const [identityMsg, setIdentityMsg] = useState<{
    kind: "success" | "error";
    text: string;
  } | null>(null);
  const [nameError, setNameError] = useState<string | null>(null);
  const [emailError, setEmailError] = useState<string | null>(null);
  const [savingCreds, setSavingCreds] = useState(false);
  const [showAdvancedCreds, setShowAdvancedCreds] = useState(false);

  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [credHost, setCredHost] = useState("");
  const [credUser, setCredUser] = useState("");
  const [credPass, setCredPass] = useState("");
  const [credMethod, setCredMethod] = useState<"https" | "ssh">("https");

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const s = await getGitStatus(projectPath);
      setStatus(s);
      if (!editingIdentity) {
        setName(s.userName ?? "");
        setEmail(s.userEmail ?? "");
      }
    } catch (e) {
      onToast("error", e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [projectPath, onToast, editingIdentity]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const ok = (message: string) => onToast("success", message);
  const fail = (message: string) => onToast("error", message);

  const fixGit = async () => {
    setConfirm(null);
    setBusyGit(true);
    try {
      const result = await installOrUpdateGit();
      if (result.success) ok(result.message);
      else fail(result.message);
      await refresh();
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
      await refresh();
    } finally {
      setBusyGit(false);
    }
  };

  const doUninstall = async () => {
    setConfirm(null);
    setBusyGit(true);
    try {
      const result = await uninstallGit();
      if (result.success) ok(result.message);
      else fail(result.message);
      await refresh();
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
      await refresh();
    } finally {
      setBusyGit(false);
    }
  };

  const connect = async (host: Host) => {
    setSigning(host);
    try {
      const result = await useGitHostAccount(host);
      if (result.success) ok(result.message);
      else fail(result.message);
      await refresh();
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
      await refresh();
    } finally {
      setSigning(null);
    }
  };

  const removeHost = async () => {
    if (confirm !== "github" && confirm !== "gitlab") return;
    const host = confirm;
    setConfirm(null);
    setRemoving(host);
    try {
      const result = await removeGitHostAccount(host);
      if (result.success) ok(result.message);
      else fail(result.message);
      await refresh();
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
      await refresh();
    } finally {
      setRemoving(null);
    }
  };

  const testHost = async (host: Host) => {
    setTesting(host);
    try {
      const result = await testGitHostConnection(host);
      if (result.success) ok(result.message);
      else onToast("info", result.message);
      await refresh();
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
    } finally {
      setTesting(null);
    }
  };

  const validateIdentity = (): boolean => {
    let okFields = true;
    if (!name.trim()) {
      setNameError("Name is required");
      okFields = false;
    } else {
      setNameError(null);
    }
    if (!isValidEmail(email)) {
      setEmailError("Enter a valid email address");
      okFields = false;
    } else {
      setEmailError(null);
    }
    return okFields;
  };

  const requestSaveIdentity = () => {
    if (!validateIdentity()) return;
    const existingName = status?.userName?.trim() ?? "";
    const existingEmail = status?.userEmail?.trim() ?? "";
    const changed =
      (existingName !== "" && existingName !== name.trim()) ||
      (existingEmail !== "" && existingEmail !== email.trim());
    if (changed) {
      setConfirm("saveIdentity");
      return;
    }
    void saveManual();
  };

  const saveManual = async () => {
    setConfirm(null);
    if (!validateIdentity()) return;
    setSavingManual(true);
    setIdentityMsg(null);
    try {
      const result = await setGitIdentity(name, email, true, null);
      if (result.success) {
        setIdentityMsg({ kind: "success", text: result.message });
        ok(result.message);
        setEditingIdentity(false);
      } else {
        setIdentityMsg({ kind: "error", text: result.message });
      }
      await refresh();
    } catch (e) {
      const text = e instanceof Error ? e.message : String(e);
      setIdentityMsg({ kind: "error", text });
      fail(text);
    } finally {
      setSavingManual(false);
    }
  };

  const clearIdentity = async () => {
    setConfirm(null);
    try {
      const result = await clearGitIdentity();
      if (result.success) {
        setIdentityMsg({ kind: "success", text: result.message });
        ok(result.message);
        setEditingIdentity(false);
      }
      await refresh();
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
    }
  };

  const saveCreds = async (overwrite: boolean) => {
    setConfirm(null);
    if (credMethod === "ssh") {
      onToast(
        "info",
        "SSH uses your system keys. Configure remotes with git@host:path and ensure your SSH key is authorized on the host.",
      );
      return;
    }
    setSavingCreds(true);
    try {
      const result = await storeGitCredentials(
        credHost,
        credUser,
        credPass,
        overwrite,
      );
      if (result.success) {
        ok(result.message);
        setCredPass("");
      } else {
        fail(result.message);
      }
    } catch (e) {
      const text = e instanceof Error ? e.message : String(e);
      if (!overwrite && text.toLowerCase().includes("already exist")) {
        setConfirm("overwriteCreds");
      } else {
        fail(text);
      }
    } finally {
      setSavingCreds(false);
    }
  };

  if (loading && !status) {
    return (
      <div className="skeleton-stack" aria-busy="true" aria-label="Checking Git">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="skeleton" />
        ))}
      </div>
    );
  }

  if (!status) {
    return (
      <div className="state-empty">
        <strong>Unable to read Git status</strong>
        <span className="resource-note">Try refreshing this page.</span>
      </div>
    );
  }

  const chip = installChip(status);
  const versionLabel = status.installed
    ? (status.versionShort ?? status.version ?? "Installed")
    : "—";
  const versionSub = !status.installed
    ? "Git is required for version control on this system"
    : status.updateAvailable
      ? `Newer package available · ${status.latestVersion}`
      : status.version ?? "Up to date";

  const identityDirty =
    editingIdentity &&
    (name.trim() !== (status.userName ?? "").trim() ||
      email.trim() !== (status.userEmail ?? "").trim());

  return (
    <div className="node-doctor">
      {/* 1. Git Installation */}
      <section className="group-box" aria-labelledby="git-install-heading">
        <div className="group-box-head">
          <h3 id="git-install-heading">Git installation</h3>
          <div className="issues-hero-actions">
            {status.installStatus === "not_installed" && (
              <button
                type="button"
                className="btn btn-primary"
                disabled={busyGit}
                onClick={() => setConfirm("install")}
              >
                {busyGit ? "Installing…" : "Install Git"}
              </button>
            )}
            {status.installStatus === "update_available" && (
              <button
                type="button"
                className="btn btn-primary"
                disabled={busyGit}
                onClick={() => setConfirm("update")}
              >
                {busyGit
                  ? "Updating…"
                  : `Update Git${status.latestVersion ? ` · ${status.latestVersion}` : ""}`}
              </button>
            )}
            {status.installed && (
              <button
                type="button"
                className="btn btn-danger"
                disabled={busyGit}
                onClick={() => setConfirm("uninstall")}
              >
                Remove Git
              </button>
            )}
          </div>
        </div>
        <div className="group-box-body" style={{ padding: 14 }}>
          <div className="git-status-row">
            <div>
              <div className="metric-label">Version</div>
              <div className="metric-value" style={{ fontSize: 18 }}>
                {loading ? "Checking…" : versionLabel}
              </div>
              <div className="metric-sub">{versionSub}</div>
            </div>
            <span
              className={`chip ${
                chip.kind === "success"
                  ? "success"
                  : chip.kind === "warning"
                    ? "warning"
                    : chip.kind === "error"
                      ? "error"
                      : ""
              }`}
            >
              {loading ? "Checking" : chip.label}
            </span>
          </div>
        </div>
      </section>

      {/* 2. Global Git Identity */}
      <section className="group-box" aria-labelledby="git-identity-heading">
        <div className="group-box-head">
          <h3 id="git-identity-heading">Global Git identity</h3>
          <div className="issues-hero-actions">
            {status.identityOk && !editingIdentity && (
              <>
                <button
                  type="button"
                  className="btn"
                  disabled={!status.installed}
                  onClick={() => {
                    setEditingIdentity(true);
                    setIdentityMsg(null);
                  }}
                >
                  Edit identity
                </button>
                <button
                  type="button"
                  className="btn"
                  onClick={() => setConfirm("clearIdentity")}
                >
                  Clear
                </button>
              </>
            )}
          </div>
        </div>
        <div className="group-box-body" style={{ padding: 14 }}>
          <p className="resource-note" style={{ margin: "0 0 12px" }}>
            Default author name and email for commits (
            <span className="mono">git config --global</span>). This is not your
            OS account and not a GitHub/GitLab login. A repository can override
            these values locally.
          </p>

          {!editingIdentity && status.identityOk ? (
            <div className="git-identity-readonly">
              <div>
                <div className="metric-label">Name</div>
                <div className="metric-value" style={{ fontSize: 15 }}>
                  {status.userName}
                </div>
              </div>
              <div>
                <div className="metric-label">Email</div>
                <div className="metric-value" style={{ fontSize: 15 }}>
                  {status.userEmail}
                </div>
              </div>
            </div>
          ) : (
            <div className="git-manual-form">
              <label htmlFor="git-global-name">
                Name
                <input
                  id="git-global-name"
                  className="dialog-input"
                  value={name}
                  onChange={(e) => {
                    setName(e.target.value);
                    setNameError(null);
                    setEditingIdentity(true);
                  }}
                  placeholder="Your name"
                  disabled={!status.installed || savingManual}
                  aria-invalid={!!nameError}
                  aria-describedby={nameError ? "git-name-error" : undefined}
                />
                {nameError && (
                  <span id="git-name-error" className="field-error">
                    {nameError}
                  </span>
                )}
              </label>
              <label htmlFor="git-global-email">
                Email
                <input
                  id="git-global-email"
                  className="dialog-input"
                  type="email"
                  value={email}
                  onChange={(e) => {
                    setEmail(e.target.value);
                    setEmailError(null);
                    setEditingIdentity(true);
                  }}
                  placeholder="you@company.com"
                  disabled={!status.installed || savingManual}
                  aria-invalid={!!emailError}
                  aria-describedby={emailError ? "git-email-error" : undefined}
                />
                {emailError && (
                  <span id="git-email-error" className="field-error">
                    {emailError}
                  </span>
                )}
              </label>
              <div className="git-identity-actions">
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={
                    !status.installed ||
                    savingManual ||
                    !name.trim() ||
                    !email.trim()
                  }
                  onClick={() => requestSaveIdentity()}
                >
                  {savingManual ? "Saving…" : "Save identity"}
                </button>
                {editingIdentity && (status.identityOk || identityDirty) && (
                  <button
                    type="button"
                    className="btn"
                    disabled={savingManual}
                    onClick={() => {
                      setEditingIdentity(false);
                      setName(status.userName ?? "");
                      setEmail(status.userEmail ?? "");
                      setNameError(null);
                      setEmailError(null);
                      setIdentityMsg(null);
                    }}
                  >
                    Cancel
                  </button>
                )}
              </div>
            </div>
          )}

          {!status.identityOk && !editingIdentity && (
            <p className="resource-note" style={{ marginTop: 8 }}>
              No global name or email set yet. Commits will prompt for an
              identity until you save one here.
            </p>
          )}

          {identityMsg && (
            <p
              className={
                identityMsg.kind === "success"
                  ? "inline-success"
                  : "inline-error"
              }
              role="status"
            >
              {identityMsg.text}
            </p>
          )}
        </div>
      </section>

      {/* 3. Git Account Connections */}
      <section className="group-box" aria-labelledby="git-accounts-heading">
        <div className="group-box-head">
          <h3 id="git-accounts-heading">Git Account Connections</h3>
        </div>
        <div className="group-box-body" style={{ padding: "10px 14px 14px" }}>
          <p className="resource-note" style={{ margin: "0 0 12px" }}>
            Manage authentication for remote Git repositories. Your Git commit
            identity is configured separately above.
          </p>
          <div className="stats-grid stats-grid-2">
            <ConnectionCard
              account={status.github}
              busy={
                signing === "github" ||
                removing === "github" ||
                testing === "github"
              }
              testing={testing === "github"}
              connecting={signing === "github"}
              gitReady={status.installed}
              onConnect={() => void connect("github")}
              onTest={() => void testHost("github")}
              onDisconnect={() => setConfirm("github")}
            />
            <ConnectionCard
              account={status.gitlab}
              busy={
                signing === "gitlab" ||
                removing === "gitlab" ||
                testing === "gitlab"
              }
              testing={testing === "gitlab"}
              connecting={signing === "gitlab"}
              gitReady={status.installed}
              onConnect={() => void connect("gitlab")}
              onTest={() => void testHost("gitlab")}
              onDisconnect={() => setConfirm("gitlab")}
            />
          </div>
        </div>
      </section>

      {/* 4. Custom Git Host */}
      <section className="group-box" aria-labelledby="git-custom-heading">
        <div className="group-box-head">
          <h3 id="git-custom-heading">Custom Git host</h3>
        </div>
        <div className="group-box-body" style={{ padding: 14 }}>
          <p className="resource-note" style={{ margin: "0 0 12px" }}>
            Self-hosted GitLab, Bitbucket, Azure DevOps, Gitea, and other
            Git-compatible hosts. HTTPS credentials go to the system credential
            helper — DevMonitor never stores tokens in its own files.
          </p>
          <div className="git-manual-form git-cred-form">
            <label htmlFor="git-cred-host">
              Host URL
              <input
                id="git-cred-host"
                className="dialog-input"
                value={credHost}
                onChange={(e) => setCredHost(e.target.value)}
                placeholder="git.example.com"
                disabled={!status.installed || savingCreds}
              />
            </label>
            <label htmlFor="git-cred-method">
              Auth method
              <select
                id="git-cred-method"
                className="dialog-input"
                value={credMethod}
                onChange={(e) =>
                  setCredMethod(e.target.value === "ssh" ? "ssh" : "https")
                }
                disabled={!status.installed || savingCreds}
              >
                <option value="https">HTTPS + token</option>
                <option value="ssh">SSH keys</option>
              </select>
            </label>
            {credMethod === "https" && (
              <>
                <label htmlFor="git-cred-user">
                  Username
                  <input
                    id="git-cred-user"
                    className="dialog-input"
                    value={credUser}
                    onChange={(e) => setCredUser(e.target.value)}
                    placeholder="username"
                    disabled={!status.installed || savingCreds}
                    autoComplete="username"
                  />
                </label>
                <label htmlFor="git-cred-pass">
                  Token / password
                  <input
                    id="git-cred-pass"
                    className="dialog-input"
                    type="password"
                    value={credPass}
                    onChange={(e) => setCredPass(e.target.value)}
                    placeholder="••••••••"
                    disabled={!status.installed || savingCreds}
                    autoComplete="new-password"
                  />
                </label>
              </>
            )}
            <button
              type="button"
              className="btn btn-primary"
              disabled={
                !status.installed ||
                savingCreds ||
                !credHost.trim() ||
                (credMethod === "https" &&
                  (!credUser.trim() || !credPass.trim()))
              }
              onClick={() => void saveCreds(false)}
            >
              {savingCreds
                ? "Saving…"
                : credMethod === "ssh"
                  ? "SSH guidance"
                  : "Save credentials"}
            </button>
          </div>
          <button
            type="button"
            className="btn"
            style={{ marginTop: 10 }}
            onClick={() => setShowAdvancedCreds((v) => !v)}
          >
            {showAdvancedCreds ? "Hide notes" : "About credential storage"}
          </button>
          {showAdvancedCreds && (
            <p className="resource-note" style={{ marginTop: 8 }}>
              Existing credentials for the same host are not overwritten without
              confirmation. Disconnecting GitHub/GitLab does not delete SSH keys
              or system credentials unless you ask. Secrets are never shown after
              save.
            </p>
          )}
        </div>
      </section>

      {confirm && (
        <div
          className="overlay"
          onClick={() => setConfirm(null)}
          role="presentation"
        >
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="git-confirm-title"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 id="git-confirm-title">
              {confirm === "uninstall"
                ? "Remove Git from this system?"
                : confirm === "install"
                  ? "Install Git?"
                  : confirm === "update"
                    ? "Update Git?"
                    : confirm === "clearIdentity"
                      ? "Clear global identity?"
                      : confirm === "saveIdentity"
                        ? "Replace global identity?"
                        : confirm === "overwriteCreds"
                          ? "Replace host credentials?"
                          : `Disconnect ${confirm === "gitlab" ? "TeamTA GitLab" : "GitHub"}?`}
            </h2>
            <div className="dialog-meta">
              {confirm === "uninstall" &&
                "Uninstalls the git package via apt (password prompt). Also clears global user.name / user.email. Host CLI sessions and SSH keys are left alone."}
              {confirm === "install" &&
                "Installs the git package with apt. You will be asked to authorize the system installer (pkexec)."}
              {confirm === "update" &&
                `Upgrades git to the apt candidate${status.latestVersion ? ` (${status.latestVersion})` : ""}. Authorize the installer when prompted.`}
              {confirm === "clearIdentity" &&
                "Removes git config --global user.name and user.email only. Host authentication is unchanged."}
              {confirm === "saveIdentity" &&
                "This replaces your existing global commit name/email. Host logins are not changed."}
              {confirm === "overwriteCreds" &&
                `Credentials already exist for ${credHost.trim()}. Replace them in the system credential helper?`}
              {(confirm === "github" || confirm === "gitlab") &&
                "Signs out of the host CLI on this machine. Global Git identity and SSH keys are not removed."}
            </div>
            <div className="dialog-actions">
              <button
                type="button"
                className="btn"
                onClick={() => setConfirm(null)}
              >
                Cancel
              </button>
              <button
                type="button"
                className={
                  confirm === "install" ||
                  confirm === "update" ||
                  confirm === "saveIdentity" ||
                  confirm === "overwriteCreds"
                    ? "btn btn-primary"
                    : "btn btn-danger"
                }
                onClick={() => {
                  if (confirm === "uninstall") void doUninstall();
                  else if (confirm === "install" || confirm === "update")
                    void fixGit();
                  else if (confirm === "clearIdentity") void clearIdentity();
                  else if (confirm === "saveIdentity") void saveManual();
                  else if (confirm === "overwriteCreds") void saveCreds(true);
                  else void removeHost();
                }}
              >
                {confirm === "uninstall"
                  ? "Remove Git"
                  : confirm === "install"
                    ? "Install"
                    : confirm === "update"
                      ? "Update"
                      : confirm === "clearIdentity"
                        ? "Clear"
                        : confirm === "saveIdentity"
                          ? "Replace"
                          : confirm === "overwriteCreds"
                            ? "Replace credentials"
                            : "Disconnect"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function ConnectionCard({
  account,
  busy,
  testing,
  connecting,
  gitReady,
  onConnect,
  onTest,
  onDisconnect,
}: {
  account: GitHostAccount;
  busy: boolean;
  testing: boolean;
  connecting: boolean;
  gitReady: boolean;
  onConnect: () => void;
  onTest: () => void;
  onDisconnect: () => void;
}) {
  const chip = authChip(account.authStatus);
  const detail = account.signedIn
    ? [account.login, account.email].filter(Boolean).join(" · ") ||
      "Authenticated via CLI"
    : account.cli
      ? `${account.cli} available · not signed in`
      : "Connect via browser when ready";

  return (
    <div
      className={`stat-box ${account.authStatus === "connected" ? "host-connected" : ""}`}
    >
      <label className="host-account-label">
        <span>{account.label}</span>
        <span
          className={`chip ${
            chip.kind === "success"
              ? "success"
              : chip.kind === "warning"
                ? "warning"
                : chip.kind === "error"
                  ? "error"
                  : ""
          }`}
        >
          {chip.label}
        </span>
      </label>
      <strong>
        {account.signedIn
          ? (account.name ?? account.login ?? "Connected")
          : "Not connected"}
      </strong>
      <span className="resource-note">{detail}</span>
      <div
        className="issues-hero-actions"
        style={{ marginTop: 10, flexWrap: "wrap" }}
      >
        {!account.signedIn ? (
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy || !gitReady}
            onClick={onConnect}
          >
            {connecting ? "Connecting…" : "Connect"}
          </button>
        ) : (
          <button
            type="button"
            className="btn"
            disabled={busy || !gitReady}
            onClick={onConnect}
          >
            {connecting ? "Refreshing…" : "Manage"}
          </button>
        )}
        <button
          type="button"
          className="btn"
          disabled={busy || !gitReady}
          onClick={onTest}
        >
          {testing ? "Testing…" : "Test connection"}
        </button>
        {account.signedIn && (
          <button
            type="button"
            className="btn btn-danger"
            disabled={busy}
            onClick={onDisconnect}
          >
            Disconnect
          </button>
        )}
      </div>
    </div>
  );
}
