import { FolderGit2, Plus, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { NewProjectDialog } from "../../components/NewProjectDialog/NewProjectDialog";
import { listProjects, type Project } from "../../services/projects";
import "./DashboardPage.css";

export function DashboardPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string>();
  const [showProjectDialog, setShowProjectDialog] = useState(false);

  const loadProjects = useCallback(async () => {
    setIsLoading(true);
    setError(undefined);
    try {
      setProjects(await listProjects());
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to load the project registry.");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadProjects();
  }, [loadProjects]);

  const handleCreated = () => {
    setShowProjectDialog(false);
    void loadProjects();
  };

  return (
    <section className="page dashboard-page">
      <header className="page-header">
        <div>
          <p className="eyebrow">Control plane / local</p>
          <h1>Projects</h1>
          <p className="muted">Git-backed workspaces registered on this machine.</p>
        </div>
        <div className="header-actions">
          <button className="icon-button" type="button" onClick={() => void loadProjects()} disabled={isLoading} aria-label="Refresh projects"><RefreshCw size={16} className={isLoading ? "spin" : undefined} /></button>
          <button className="primary-button" type="button" onClick={() => setShowProjectDialog(true)}>
            <Plus size={16} /> New project
          </button>
        </div>
      </header>

      {error && <div className="inline-error" role="alert">{error}</div>}

      {isLoading ? (
        <div className="empty-state"><span className="empty-index">SYNC</span><h2>Loading project registry</h2></div>
      ) : projects.length === 0 ? (
        <div className="empty-state">
          <span className="empty-index">M1</span>
          <h2>Your project registry is empty</h2>
          <p>Create a new Git repository or register an existing local repository.</p>
          <button className="primary-button" type="button" onClick={() => setShowProjectDialog(true)}><Plus size={16} /> Add first project</button>
        </div>
      ) : (
        <div className="project-grid">
          {projects.map((project) => <ProjectCard key={project.id} project={project} />)}
        </div>
      )}

      {showProjectDialog && <NewProjectDialog onClose={() => setShowProjectDialog(false)} onCreated={handleCreated} />}
    </section>
  );
}

function ProjectCard({ project }: { project: Project }) {
  const workspace = project.workspaces[0];
  return (
    <Link className="project-card" to={`/projects/${project.id}`}>
      <div className="project-card-icon"><FolderGit2 size={18} /></div>
      <div className="project-card-heading">
        <h2>{project.name}</h2>
        <span className="branch-chip">{project.defaultBranch}</span>
      </div>
      <p>{project.description || "No project description."}</p>
      <div className="project-workspace" title={workspace?.path}>
        <span>workspace</span>
        <code>{workspace?.path || "No local workspace"}</code>
      </div>
    </Link>
  );
}
