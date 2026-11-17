import { computed, shallowRef, watch, type ComputedRef } from 'vue';
import { acknowledged, useBerth } from './berth';

const OPEN_KEY = 'berth.workspace.open';
const chosenWorkspace = shallowRef<string | null>(localStorage.getItem(OPEN_KEY));

watch(chosenWorkspace, (id) => (id ? localStorage.setItem(OPEN_KEY, id) : localStorage.removeItem(OPEN_KEY)));

export function useOpenWorkspace(): ComputedRef<string | null> {
  const berth = useBerth();
  return computed(() => {
    const id = chosenWorkspace.value;
    return id && berth?.entities.value.workspaces.has(id) ? id : null;
  });
}

export async function openWorkspace(id: string | null): Promise<void> {
  if (id) await acknowledged({ OpenWorkspace: { id } });
  chosenWorkspace.value = id;
}

export type NewProject = { path: string; name: string | null; createMissing: boolean };

export async function addWorkspace(name: string): Promise<void> {
  await acknowledged({ AddWorkspace: { name } });
}

export async function removeWorkspace(id: string): Promise<void> {
  await acknowledged({ RemoveWorkspace: { id } });
}

export async function renameWorkspace(id: string, name: string): Promise<void> {
  await acknowledged({ RenameWorkspace: { id, name } });
}

export async function addProject(workspace: string, project: NewProject): Promise<void> {
  await acknowledged({ OpenWorkspace: { id: workspace } });
  await acknowledged({ AddProject: { path: project.path, name: project.name, create_missing: project.createMissing } });
}

export async function removeProject(id: string): Promise<void> {
  await acknowledged({ RemoveProject: { id } });
}

export async function renameProject(id: string, name: string): Promise<void> {
  await acknowledged({ RenameProject: { id, name } });
}

export async function setProjectPins(order: string[]): Promise<void> {
  await acknowledged({ SetProjectPins: { order } });
}
