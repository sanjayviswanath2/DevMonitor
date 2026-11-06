import { formatBytes } from "../lib/format";

type Props = {
  name: string;
  pid: number;
  port?: number;
  memory?: number;
  killing: boolean;
  onCancel: () => void;
  onConfirm: () => void;
};

export function ConfirmDialog({
  name,
  pid,
  port,
  memory,
  killing,
  onCancel,
  onConfirm,
}: Props) {
  return (
    <div className="overlay" onClick={onCancel} role="presentation">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="kill-title"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="kill-title">Terminate {name}?</h2>
        <div className="dialog-meta">
          <div>
            <span>PID</span>
            {pid}
          </div>
          {port != null && (
            <div>
              <span>Port</span>
              {port}
            </div>
          )}
          {memory != null && (
            <div>
              <span>Memory</span>
              {formatBytes(memory)}
            </div>
          )}
        </div>
        <div className="dialog-actions">
          <button className="btn" onClick={onCancel} disabled={killing}>
            Cancel
          </button>
          <button
            className="btn btn-danger"
            onClick={onConfirm}
            disabled={killing}
          >
            {killing ? "Terminating…" : "Terminate"}
          </button>
        </div>
      </div>
    </div>
  );
}
