import { ArrowLeft, Plus, RefreshCw, Save, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent, type ReactNode } from "react";
import { Link, useParams } from "react-router-dom";
import {
  deleteModelPricing,
  getProjectMetrics,
  microsToUsd,
  updateProjectCostControl,
  upsertModelPricing,
  usdToMicros,
  type ProjectMetrics,
} from "../../services/metrics";
import { getProject, type Project } from "../../services/projects";
import "./ProjectMetricsPage.css";

const ranges = [7, 30, 90];

export function ProjectMetricsPage() {
  const { projectId } = useParams();
  const [project, setProject] = useState<Project | null>();
  const [metrics, setMetrics] = useState<ProjectMetrics>();
  const [rangeDays, setRangeDays] = useState(30);
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  const load = useCallback(async () => {
    if (!projectId) return;
    setIsLoading(true);
    setError(undefined);
    try {
      const [loadedProject, loadedMetrics] = await Promise.all([
        getProject(projectId),
        getProjectMetrics(projectId, rangeDays),
      ]);
      setProject(loadedProject);
      setMetrics(loadedMetrics);
    } catch (loadError) {
      setError(messageFrom(loadError, "Unable to load project metrics."));
    } finally {
      setIsLoading(false);
    }
  }, [projectId, rangeDays]);

  useEffect(() => { void load(); }, [load]);

  const saveBudget = async (input: BudgetInput) => {
    if (!projectId) return;
    setIsSaving(true);
    try {
      await updateProjectCostControl({ projectId, ...input });
      await load();
    } catch (saveError) {
      setError(messageFrom(saveError, "Unable to save cost controls."));
    } finally {
      setIsSaving(false);
    }
  };

  const savePricing = async (input: PricingInput) => {
    if (!projectId) return;
    setIsSaving(true);
    try {
      await upsertModelPricing({ projectId, ...input });
      await load();
    } catch (saveError) {
      setError(messageFrom(saveError, "Unable to save model pricing."));
    } finally {
      setIsSaving(false);
    }
  };

  const removePricing = async (provider: string, model: string) => {
    if (!projectId) return;
    setIsSaving(true);
    try {
      await deleteModelPricing(projectId, provider, model);
      await load();
    } catch (saveError) {
      setError(messageFrom(saveError, "Unable to remove model pricing."));
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading && !metrics) return <LoadingState />;
  if (!project) return <NotFoundState />;

  return <section className="page project-metrics-page">
    <header className="page-header metrics-header">
      <div>
        <Link className="back-link" to={`/projects/${project.id}`}><ArrowLeft size={15} /> Board</Link>
        <p className="eyebrow">Delivery telemetry / {project.defaultBranch}</p>
        <h1>{project.name} metrics</h1>
        <p className="muted">Reliability, cost, and flow diagnostics. Integrated outcomes remain the measure of progress.</p>
      </div>
      <div className="metrics-header-actions">
        <div className="range-switch" aria-label="Metrics range">
          {ranges.map((range) => <button key={range} type="button" className={rangeDays === range ? "active" : ""} onClick={() => setRangeDays(range)}>{range}d</button>)}
        </div>
        <button className="icon-button" type="button" onClick={() => void load()} aria-label="Refresh metrics"><RefreshCw size={16} /></button>
      </div>
    </header>

    {error && <div className="inline-error" role="alert">{error}</div>}
    {metrics && <>
      <BudgetStrip metrics={metrics} />
      <MetricSection title="Operational reliability" eyebrow={`${metrics.rangeDays} day execution window`}>
        <div className="metric-grid operational-grid">
          <Metric label="Runs" value={formatInteger(metrics.operational.runCount)} />
          <Metric label="Success" value={formatPercent(metrics.operational.successRatePercent)} tone={rateTone(metrics.operational.successRatePercent, false)} />
          <Metric label="Failed" value={formatInteger(metrics.operational.failedRuns)} tone={metrics.operational.failedRuns > 0 ? "danger" : "good"} />
          <Metric label="Retries" value={formatInteger(metrics.operational.retryCount)} />
          <Metric label="Avg duration" value={formatDuration(metrics.operational.averageDurationSeconds)} />
          <Metric label="Est. cost" value={formatUsd(metrics.operational.estimatedCostMicros)} />
          <Metric label="Tokens" value={formatTokens(metrics.operational.inputTokens + metrics.operational.outputTokens)} />
        </div>
      </MetricSection>

      <MetricSection title="Flow bottlenecks" eyebrow="Time between durable workflow transitions">
        <div className="metric-grid flow-grid">
          <Metric label="Ready lead time" value={formatOptionalDuration(metrics.flow.readyLeadTimeSeconds)} />
          <Metric label="In progress" value={formatOptionalDuration(metrics.flow.inProgressSeconds)} />
          <Metric label="Review queue" value={formatOptionalDuration(metrics.flow.reviewQueueSeconds)} />
          <Metric label="Integration queue" value={formatOptionalDuration(metrics.flow.integrationQueueSeconds)} />
          <Metric label="Blocked time" value={formatOptionalDuration(metrics.flow.blockedSeconds)} />
          <Metric label="Conflict rate" value={formatPercent(metrics.flow.conflictRatePercent)} tone={rateTone(metrics.flow.conflictRatePercent, true)} />
          <Metric label="Validation failures" value={formatPercent(metrics.flow.validationFailureRatePercent)} tone={rateTone(metrics.flow.validationFailureRatePercent, true)} />
          <Metric label="Done throughput" value={`${formatInteger(metrics.flow.milestoneThroughput)} tasks`} tone="good" />
        </div>
        <p className="metrics-footnote">Stage timing uses the transition ledger introduced with M27; older tasks contribute from their first recorded state onward.</p>
      </MetricSection>

      <div className="metrics-split">
        <MetricSection title="Agent reliability" eyebrow="Provider and model attribution">
          <AgentTable metrics={metrics} />
        </MetricSection>
        <MetricSection title="Worker utilization" eyebrow="Busy time normalized by worker capacity">
          <WorkerTable metrics={metrics} />
        </MetricSection>
      </div>

      <div className="metrics-cost-layout">
        <div className="metrics-cost-main">
          <MetricSection title="Cost attribution" eyebrow="Provider-reported usage × configured rates">
            <CostTable metrics={metrics} />
          </MetricSection>
        </div>
        <aside className="metrics-controls">
          <BudgetForm metrics={metrics} isSaving={isSaving} onSubmit={saveBudget} />
          <PricingForm isSaving={isSaving} onSubmit={savePricing} />
          <PricingList metrics={metrics} isSaving={isSaving} onRemove={removePricing} />
        </aside>
      </div>
    </>}
  </section>;
}

type BudgetInput = { monthlyBudgetMicros: number; warningThresholdPercent: number; blockNewRuns: boolean };
type PricingInput = { provider: string; model: string; inputMicrosPerMillion: number; cachedInputMicrosPerMillion: number; outputMicrosPerMillion: number };

function BudgetStrip({ metrics }: { metrics: ProjectMetrics }) {
  const utilization = Math.min(metrics.budgetUtilizationPercent ?? 0, 100);
  return <section className={`budget-strip ${metrics.budgetStatus}`} aria-label="Monthly cost status">
    <div><span>Current month</span><strong>{formatUsd(metrics.currentMonthCostMicros)}</strong></div>
    <div><span>Monthly budget</span><strong>{metrics.costControl.monthlyBudgetMicros === 0 ? "Not set" : formatUsd(metrics.costControl.monthlyBudgetMicros)}</strong></div>
    <div><span>Budget state</span><strong>{metrics.budgetStatus.replace("_", " ")}</strong></div>
    <div className="budget-meter"><i style={{ width: `${utilization}%` }} /></div>
  </section>;
}

function MetricSection({ title, eyebrow, children }: { title: string; eyebrow: string; children: ReactNode }) {
  return <section className="metrics-section"><div className="metrics-section-heading"><div><p className="eyebrow">{eyebrow}</p><h2>{title}</h2></div></div>{children}</section>;
}

function Metric({ label, value, tone = "neutral" }: { label: string; value: string; tone?: string }) {
  return <div className={`telemetry-metric ${tone}`}><span>{label}</span><strong>{value}</strong></div>;
}

function AgentTable({ metrics }: { metrics: ProjectMetrics }) {
  if (metrics.agents.length === 0) return <EmptyMetrics text="No agent runs in this window." />;
  return <div className="telemetry-table-wrap"><table className="telemetry-table"><thead><tr><th>Agent</th><th>Runs</th><th>Success</th><th>Avg</th><th>Cost</th></tr></thead><tbody>{metrics.agents.map((agent) => <tr key={agent.agentId}><td><strong>{agent.agentName}</strong><code>{agent.provider} / {agent.model}</code></td><td>{agent.runCount}</td><td>{formatPercent(agent.successRatePercent)}</td><td>{formatDuration(agent.averageDurationSeconds)}</td><td>{formatUsd(agent.estimatedCostMicros)}</td></tr>)}</tbody></table></div>;
}

function WorkerTable({ metrics }: { metrics: ProjectMetrics }) {
  if (metrics.workers.length === 0) return <EmptyMetrics text="No worker activity in this window." />;
  return <div className="worker-list">{metrics.workers.map((worker) => <article key={worker.workerId}><div><strong>{worker.workerName}</strong><span>{worker.runCount} runs · {formatDuration(worker.busySeconds)} busy</span></div><em>{formatPercent(worker.utilizationPercent)}</em><div className="utilization-meter"><i style={{ width: `${worker.utilizationPercent}%` }} /></div></article>)}</div>;
}

function CostTable({ metrics }: { metrics: ProjectMetrics }) {
  if (metrics.costs.length === 0) return <EmptyMetrics text="No provider usage has been reported in this window." />;
  return <div className="telemetry-table-wrap"><table className="telemetry-table cost-table"><thead><tr><th>Provider / model</th><th>Input</th><th>Cached</th><th>Output</th><th>Estimate</th></tr></thead><tbody>{metrics.costs.map((cost) => <tr key={`${cost.provider}:${cost.model}`} className={cost.priced ? "" : "unpriced"}><td><strong>{cost.provider}</strong><code>{cost.model}</code></td><td>{formatTokens(cost.inputTokens)}</td><td>{formatTokens(cost.cachedInputTokens)}</td><td>{formatTokens(cost.outputTokens)}</td><td>{cost.priced ? formatUsd(cost.estimatedCostMicros) : "Rate needed"}</td></tr>)}</tbody></table></div>;
}

function BudgetForm({ metrics, isSaving, onSubmit }: { metrics: ProjectMetrics; isSaving: boolean; onSubmit: (input: BudgetInput) => Promise<void> }) {
  const [budget, setBudget] = useState("0");
  const [warning, setWarning] = useState("80");
  const [block, setBlock] = useState(false);
  useEffect(() => {
    setBudget(String(microsToUsd(metrics.costControl.monthlyBudgetMicros)));
    setWarning(String(metrics.costControl.warningThresholdPercent));
    setBlock(metrics.costControl.blockNewRuns);
  }, [metrics.costControl]);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    void onSubmit({ monthlyBudgetMicros: usdToMicros(Number(budget)), warningThresholdPercent: Number(warning), blockNewRuns: block });
  };
  return <form className="metrics-form" onSubmit={submit}><h2>Monthly control</h2><label>Budget (USD)<input type="number" min="0" step="0.01" value={budget} onChange={(event) => setBudget(event.target.value)} /></label><label>Warn at (%)<input type="number" min="1" max="100" value={warning} onChange={(event) => setWarning(event.target.value)} required /></label><label className="checkbox-row"><input type="checkbox" checked={block} onChange={(event) => setBlock(event.target.checked)} /><span>Pause new runs when exhausted</span></label><button className="primary-button" type="submit" disabled={isSaving}><Save size={14} /> Save control</button></form>;
}

function PricingForm({ isSaving, onSubmit }: { isSaving: boolean; onSubmit: (input: PricingInput) => Promise<void> }) {
  const [provider, setProvider] = useState("codex");
  const [model, setModel] = useState("");
  const [inputRate, setInputRate] = useState("0");
  const [cachedRate, setCachedRate] = useState("0");
  const [outputRate, setOutputRate] = useState("0");
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    await onSubmit({ provider, model, inputMicrosPerMillion: usdToMicros(Number(inputRate)), cachedInputMicrosPerMillion: usdToMicros(Number(cachedRate)), outputMicrosPerMillion: usdToMicros(Number(outputRate)) });
    setModel("");
  };
  return <form className="metrics-form" onSubmit={(event) => void submit(event)}><h2>Model rate</h2><p>USD per 1M tokens. Use <code>*</code> as a provider fallback.</p><label>Provider<input value={provider} onChange={(event) => setProvider(event.target.value)} required /></label><label>Model<input value={model} onChange={(event) => setModel(event.target.value)} placeholder="gpt-model or *" required /></label><div className="rate-grid"><label>Input<input type="number" min="0" step="0.000001" value={inputRate} onChange={(event) => setInputRate(event.target.value)} /></label><label>Cached<input type="number" min="0" step="0.000001" value={cachedRate} onChange={(event) => setCachedRate(event.target.value)} /></label><label>Output<input type="number" min="0" step="0.000001" value={outputRate} onChange={(event) => setOutputRate(event.target.value)} /></label></div><button className="secondary-button" type="submit" disabled={isSaving}><Plus size={14} /> Save rate</button></form>;
}

function PricingList({ metrics, isSaving, onRemove }: { metrics: ProjectMetrics; isSaving: boolean; onRemove: (provider: string, model: string) => Promise<void> }) {
  if (metrics.pricing.length === 0) return null;
  return <section className="pricing-list"><h2>Configured rates</h2>{metrics.pricing.map((rate) => <div key={`${rate.provider}:${rate.model}`}><span><strong>{rate.provider}</strong><code>{rate.model}</code></span><em>{formatRate(rate.inputMicrosPerMillion)} / {formatRate(rate.cachedInputMicrosPerMillion)} / {formatRate(rate.outputMicrosPerMillion)}</em><button className="icon-button" type="button" disabled={isSaving} onClick={() => void onRemove(rate.provider, rate.model)} aria-label={`Remove ${rate.provider} ${rate.model} pricing`}><Trash2 size={13} /></button></div>)}</section>;
}

function EmptyMetrics({ text }: { text: string }) { return <p className="metrics-empty">{text}</p>; }
function LoadingState() { return <section className="page"><div className="empty-state"><span className="empty-index">SYNC</span><h2>Loading delivery telemetry</h2></div></section>; }
function NotFoundState() { return <section className="page"><div className="empty-state"><h2>Project not found</h2><Link className="secondary-button" to="/projects">Return to projects</Link></div></section>; }
function messageFrom(error: unknown, fallback: string) { return error instanceof Error ? error.message : fallback; }
function formatInteger(value: number) { return new Intl.NumberFormat().format(value); }
function formatTokens(value: number) { return new Intl.NumberFormat(undefined, { notation: value >= 10_000 ? "compact" : "standard", maximumFractionDigits: 1 }).format(value); }
function formatUsd(micros: number) { return new Intl.NumberFormat(undefined, { style: "currency", currency: "USD", minimumFractionDigits: 2, maximumFractionDigits: 4 }).format(microsToUsd(micros)); }
function formatRate(micros: number) { return `$${microsToUsd(micros).toFixed(3)}`; }
function formatPercent(value: number) { return `${value.toFixed(1)}%`; }
function formatOptionalDuration(value: number | null) { return value === null ? "No data" : formatDuration(value); }
function formatDuration(seconds: number) { if (seconds < 60) return `${Math.round(seconds)}s`; if (seconds < 3600) return `${Math.round(seconds / 60)}m`; if (seconds < 86400) return `${(seconds / 3600).toFixed(1)}h`; return `${(seconds / 86400).toFixed(1)}d`; }
function rateTone(value: number, lowerIsBetter: boolean) { const good = lowerIsBetter ? value < 10 : value >= 90; const danger = lowerIsBetter ? value >= 25 : value < 60; return good ? "good" : danger ? "danger" : "warning"; }
