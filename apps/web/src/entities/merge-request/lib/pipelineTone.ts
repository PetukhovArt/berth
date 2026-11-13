import type { PipelineStatus } from '../../../shared/api/connectors/api';

const PIPELINE_TONE: Record<PipelineStatus, 'success' | 'crashed' | 'running' | 'idle'> = {
  success: 'success',
  failed: 'crashed',
  running: 'running',
  pending: 'idle',
  canceled: 'idle',
  none: 'idle',
};

export function pipelineTone(status: PipelineStatus): 'success' | 'crashed' | 'running' | 'idle' {
  return PIPELINE_TONE[status];
}
