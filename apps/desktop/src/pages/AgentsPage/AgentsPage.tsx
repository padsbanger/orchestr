import { Bot, Pencil, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { AgentDialog } from "../../components/AgentDialog/AgentDialog";
import { createAgent, deleteAgent, listAgents, type Agent, type AgentInput, updateAgent } from "../../services/agents";
import "./AgentsPage.css";

export function AgentsPage() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [editingAgent, setEditingAgent] = useState<Agent | null>();
  const [isCreating, setIsCreating] = useState(false);

  const loadAgents = useCallback(async () => {
    setIsLoading(true); setError(undefined);
    try { setAgents(await listAgents()); }
    catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Unable to load agents."); }
    finally { setIsLoading(false); }
  }, []);
  useEffect(() => { void loadAgents(); }, [loadAgents]);

  const saveAgent = async (input: AgentInput) => {
    if (editingAgent) await updateAgent(editingAgent.id, input); else await createAgent(input);
    await loadAgents();
  };
  const removeAgent = async (agent: Agent) => {
    if (!window.confirm(`Delete ${agent.name}? Tasks assigned to it will become unassigned.`)) return;
    try { await deleteAgent(agent.id); await loadAgents(); }
    catch (deleteError) { setError(deleteError instanceof Error ? deleteError.message : "Unable to delete agent."); }
  };

  return <section className="page agents-page">
    <header className="page-header"><div><p className="eyebrow">Agent configurations</p><h1>Agents</h1><p className="muted">Agents define roles and instructions. Provider sign-in remains on the worker.</p></div><button className="primary-button" type="button" onClick={() => { setEditingAgent(null); setIsCreating(true); }}><Plus size={16} /> New agent</button></header>
    {error && <div className="inline-error" role="alert">{error}</div>}
    {isLoading ? <div className="empty-state"><span className="empty-index">SYNC</span><h2>Loading agents</h2></div> : agents.length === 0 ? <div className="empty-state"><span className="empty-index">M7</span><h2>No agents configured</h2><p>Create a Codex agent configuration, then assign it to tasks from a board.</p><button className="primary-button" type="button" onClick={() => setIsCreating(true)}><Plus size={16} /> Add first agent</button></div> : <div className="agent-grid">{agents.map((agent) => <article className="agent-card" key={agent.id}><div className="agent-card-icon"><Bot size={18} /></div><div className="agent-card-heading"><div><h2>{agent.name}</h2><p>{agent.role}</p></div><span>{agent.provider}</span></div><dl><div><dt>Model</dt><dd>{agent.model || "Provider default"}</dd></div><div><dt>Concurrency</dt><dd>{agent.maxConcurrentTasks}</dd></div></dl>{agent.skills.length > 0 && <div className="agent-skills">{agent.skills.map((skill) => <code key={skill}>{skill}</code>)}</div>}<footer><button type="button" onClick={() => setEditingAgent(agent)}><Pencil size={13} /> Edit</button><button type="button" onClick={() => void removeAgent(agent)}><Trash2 size={13} /> Delete</button></footer></article>)}</div>}
    {(isCreating || editingAgent) && <AgentDialog agent={editingAgent ?? undefined} onClose={() => { setIsCreating(false); setEditingAgent(null); }} onSave={saveAgent} />}
  </section>;
}
