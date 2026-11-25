const KEEP = {
  assistant: ['type', 'message', 'parent_tool_use_id', 'session_id', 'uuid'],
  user: ['type', 'message', 'parent_tool_use_id', 'session_id', 'uuid'],
  stream_event: ['type', 'event', 'parent_tool_use_id', 'session_id', 'uuid', 'api_message_id'],
  result: ['type', 'subtype', 'is_error', 'stop_reason', 'terminal_reason', 'result', 'session_id', 'uuid'],
};
const INIT_KEEP = ['type', 'subtype', 'session_id', 'model', 'permissionMode', 'claude_code_version', 'uuid'];
const MODELS_KEPT = ['default', 'sonnet', 'opus', 'haiku'];
const COMMANDS_KEPT = 3;
const DENIED = 'The user denied this tool call.';

const pick = (object, keys) => Object.fromEntries(keys.filter((k) => k in object).map((k) => [k, object[k]]));

const withoutKeys = (value, names) => {
  if (Array.isArray(value)) return value.map((v) => withoutKeys(v, names));
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([k]) => !names.includes(k))
        .map(([k, v]) => [k, withoutKeys(v, names)]),
    );
  }
  return value;
};

const ranges = (spec) =>
  spec
    .split(',')
    .filter(Boolean)
    .map((range) => range.split('-').map(Number));

export const claudeWorkspace = (parts) => parts.flatMap((p) => p.records).find((r) => r.dir === 'meta' && r.cwd)?.cwd;

const select = (part) => {
  const lines = part.records
    .map((record, index) => ({ index, ...record }))
    .filter((record) => record.dir === 'stdin' || record.dir === 'stdout')
    .map((record) => ({ index: record.index, dir: record.dir, frame: JSON.parse(record.line) }));
  if (!part.lines) return lines;
  return ranges(part.lines).flatMap(([from, to = from]) => lines.filter((l) => l.index >= from && l.index <= to));
};

const slimOut = (frame) => {
  if (frame.type === 'system') return frame.subtype === 'init' ? pick(frame, INIT_KEEP) : null;
  if (frame.type === 'rate_limit_event') return null;
  if (frame.type === 'control_response') {
    const body = frame.response.response;
    if (!body?.models) return frame;
    return {
      ...frame,
      response: {
        ...frame.response,
        response: {
          models: body.models.filter((m) => MODELS_KEPT.includes(m.value)).map((m) => pick(m, ['value', 'displayName', 'description', 'supportedEffortLevels'])),
          commands: body.commands.slice(0, COMMANDS_KEPT).map((c) => pick(c, ['name', 'description'])),
        },
      },
    };
  }
  const kept = KEEP[frame.type] ? pick(frame, KEEP[frame.type]) : frame;
  return withoutKeys(kept, ['usage', 'signature', 'diagnostics', 'context_management', 'input_transformations', 'caller']);
};

const userBlocks = (message) => {
  const content = typeof message.content === 'string' ? [{ type: 'text', text: message.content }] : message.content;
  const text = content.filter((b) => b.type === 'text').map((b) => b.text).join('\n');
  const berthOrder = [...content.filter((b) => b.type === 'text'), ...content.filter((b) => b.type !== 'text')];
  const attachments = content
    .filter((b) => b.type === 'image')
    .map((b) => ({ Image: { mime: b.source.media_type, name: 'image.png', data: b.source.data } }));
  return { content: berthOrder, text, attachments };
};

const answerFor = (pending, response) => {
  if (!pending) throw new Error('an answer without a request');
  if (pending.questions) {
    const given = response.updatedInput?.answers ?? {};
    const form = {};
    pending.questions.forEach((q, i) => {
      if (given[q.question] !== undefined) form[`q${i}`] = String(given[q.question]).split(', ');
    });
    return { form };
  }
  if (response.behavior === 'deny') return { option: pending.always ? 2 : 1 };
  return { option: response.updatedPermissions ? 1 : 0 };
};

export const claudeSteps = ({ parts, scrub, scrubText, provider, scenario }) => {
  const all = parts.flatMap((p) => p.records);
  const initFrame = all
    .filter((r) => r.dir === 'stdout')
    .map((r) => JSON.parse(r.line))
    .find((f) => f.type === 'system' && f.subtype === 'init');
  const steps = [
    {
      type: 'transcript_start',
      provider,
      driver: 'claude',
      protocol: 'stream-json',
      version: initFrame?.claude_code_version ?? 'unknown',
      scenario,
    },
    { type: 'berth', call: { call: 'create', args: { name: 'a' } } },
  ];
  let controls = 0;
  for (const part of parts) {
    const at = part.records.find((r) => r.dir === 'meta' && r.cmd)?.cmd?.match(/--resume-session-at (\S+)/)?.[1];
    if (at) steps.push({ type: 'berth', call: { call: 'rewind', args: { to: at } } });
    const lines = select(part);
    const ids = new Map();
    let pending = null;
    let started = false;
    const promptFor = (user) => {
      const { text, attachments } = userBlocks(user.frame.message);
      const args = { text: scrubText(text) };
      if (attachments.length > 0) args.attachments = attachments;
      steps.push({ type: 'berth', call: { call: 'prompt', args } });
    };
    const firstUser = lines.find((l) => l.dir === 'stdin' && l.frame.type === 'user');
    for (const line of lines) {
      const { dir, frame } = line;
      if (dir === 'stdout') {
        if (frame.type === 'control_request') {
          const request = frame.request;
          pending = {
            always: (request.permission_suggestions ?? []).length > 0,
            questions: request.tool_name === 'AskUserQuestion' ? request.input.questions : null,
          };
        }
        const slim = slimOut(structuredClone(frame));
        if (!slim) continue;
        if (slim.type === 'control_response' && ids.has(slim.response.request_id)) slim.response.request_id = ids.get(slim.response.request_id);
        steps.push({ type: 'emit_inbound', label: slim.subtype ? `${slim.type}/${slim.subtype}` : slim.type, frame: scrub(slim) });
        continue;
      }
      if (!started) {
        started = true;
        if (firstUser) promptFor(firstUser);
      }
      let expected = frame;
      if (frame.type === 'user') {
        if (line !== firstUser) promptFor(line);
        expected = { type: 'user', message: { role: 'user', content: userBlocks(frame.message).content } };
      } else if (frame.type === 'control_request') {
        const request = frame.request;
        const id = `b${++controls}`;
        ids.set(frame.request_id, id);
        expected = { ...frame, request_id: id };
        if (request.subtype === 'set_model') steps.push({ type: 'berth', call: { call: 'set_option', args: { id: 'model', value: request.model } } });
        else if (request.subtype === 'apply_flag_settings') steps.push({ type: 'berth', call: { call: 'set_option', args: { id: 'effort', value: request.settings.effortLevel } } });
        else if (request.subtype === 'interrupt') steps.push({ type: 'berth', call: { call: 'stop' } });
        else if (request.subtype !== 'initialize') throw new Error(`no berth call produces ${request.subtype}`);
      } else if (frame.type === 'control_response') {
        steps.push({ type: 'berth', call: { call: 'answer', args: answerFor(pending, frame.response.response) } });
        if (!pending.questions && frame.response.response.behavior === 'deny') {
          expected = structuredClone(frame);
          expected.response.response.message = DENIED;
        }
        pending = null;
      }
      steps.push({ type: 'expect_outbound', label: expected.type, frame: scrub(structuredClone(expected)) });
    }
  }
  return steps;
};
