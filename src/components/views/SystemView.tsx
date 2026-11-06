import type { ComponentProps } from "react";
import { CpuView } from "./CpuView";
import { MemoryView } from "./MemoryView";
import { ProcessesView } from "./ProcessesView";

type Props = {
  cpu: Omit<ComponentProps<typeof CpuView>, "embedded">;
  memory: ComponentProps<typeof MemoryView>;
  processes: ComponentProps<typeof ProcessesView>;
};

export function SystemView({ cpu, memory, processes }: Props) {
  return (
    <div className="system-page">
      <section className="system-group" aria-labelledby="system-resources-heading">
        <header className="system-group-head">
          <h3 id="system-resources-heading">Resources</h3>
          <p>CPU and memory over the last minute</p>
        </header>
        <div className="system-group-body system-resources">
          <div className="system-col">
            <CpuView {...cpu} embedded />
          </div>
          <div className="system-col">
            <MemoryView {...memory} />
          </div>
        </div>
      </section>

      <section className="system-group" aria-labelledby="system-running-heading">
        <header className="system-group-head">
          <h3 id="system-running-heading">Running</h3>
          <p>Apps and processes on this machine</p>
        </header>
        <div className="system-group-body">
          <ProcessesView {...processes} />
        </div>
      </section>
    </div>
  );
}
