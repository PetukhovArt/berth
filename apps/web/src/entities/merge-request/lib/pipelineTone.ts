import type { PipelineStatus } from '../../../shared/api/connectors/api';

type PipelineTone = 'success' | 'crashed' | 'running' | 'pending' | 'canceled' | 'idle';

const PIPELINE_TONE: Record<PipelineStatus, PipelineTone> = {
  success: 'success',
  failed: 'crashed',
  running: 'running',
  pending: 'pending',
  canceled: 'canceled',
  none: 'idle',
};

export function pipelineTone(status: PipelineStatus): PipelineTone {
  return PIPELINE_TONE[status];
}
