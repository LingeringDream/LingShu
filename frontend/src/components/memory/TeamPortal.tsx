import { useEffect } from 'react';
import { CloudAccountPanel } from '../settings/CloudAccountPanel';
import { useCloudAccountStore } from '../../stores/cloudAccountStore';
import { TeamWorkspace } from './TeamWorkspace';

/** Standalone shared-server UI; deliberately never bootstraps a local session. */
export function TeamPortal() {
  const initialize = useCloudAccountStore(s => s.initialize);
  useEffect(() => { void initialize(); }, [initialize]);
  return <main className="team-portal">
    <h1>灵枢 · 团队记忆控制台</h1>
    <CloudAccountPanel />
    <TeamWorkspace />
  </main>;
}
