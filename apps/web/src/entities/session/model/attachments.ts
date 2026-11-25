import { acpAttachment, type AcpAttachment, type AcpAttachmentUpload } from './berth';

export const ATTACH_BUDGET = 3 * 1024 * 1024;
export const MAX_IMAGE_SOURCE_BYTES = 30 * 1024 * 1024;
export const MAX_TEXT_BYTES = 2 * 1024 * 1024;
const IMAGE_MAX_SIDE = 2048;
const IMAGE_QUALITY = 0.85;

const sources = new Map<string, Promise<string>>();

export function attachmentSrc(attachment: Extract<AcpAttachment, { Image: unknown }>): Promise<string> {
  const { blob, mime } = attachment.Image;
  let hit = sources.get(blob);
  if (!hit) {
    hit = acpAttachment(blob).then((data) => `data:${mime};base64,${data}`);
    sources.set(blob, hit);
    hit.catch(() => sources.delete(blob));
  }
  return hit;
}

export const uploadSrc = (upload: AcpAttachmentUpload): string | null => ('Image' in upload ? `data:${upload.Image.mime};base64,${upload.Image.data}` : null);

export const uploadName = (upload: AcpAttachmentUpload): string => ('Image' in upload ? upload.Image.name : 'File' in upload ? upload.File.name : upload.WorktreeFile.path);

export const attachmentName = (attachment: AcpAttachment): string => ('Image' in attachment ? attachment.Image.name : 'File' in attachment ? attachment.File.name : attachment.WorktreeFile.path);

function base64Of(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(',', 2)[1] ?? '');
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

export type UploadFailure = 'notText' | 'overBudget';

export const uploadWeight = (upload: AcpAttachmentUpload): number => ('Image' in upload ? upload.Image.data.length : 'File' in upload ? new TextEncoder().encode(upload.File.text).length : upload.WorktreeFile.path.length);

async function shrunk(file: File, room: number): Promise<AcpAttachmentUpload | null> {
  const bitmap = await createImageBitmap(file).catch(() => null);
  if (!bitmap) return null;
  const scale = Math.min(1, IMAGE_MAX_SIDE / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  const context = canvas.getContext('2d');
  if (!context) return null;
  context.fillStyle = '#fff';
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/jpeg', IMAGE_QUALITY));
  const data = blob ? await base64Of(blob) : '';
  return data && data.length <= room ? { Image: { mime: 'image/jpeg', name: file.name.replace(/\.[^.]*$/, '') + '.jpg', data } } : null;
}

export async function fileUpload(file: File, room: number): Promise<AcpAttachmentUpload | UploadFailure> {
  if (file.type.startsWith('image/')) {
    if (4 * Math.ceil(file.size / 3) <= room) return { Image: { mime: file.type, name: file.name, data: await base64Of(file) } };
    return (await shrunk(file, room)) ?? 'overBudget';
  }
  const text = await file.text();
  if (text.includes('\u0000')) return 'notText';
  const upload = { File: { mime: file.type || 'text/plain', name: file.name, text } };
  return uploadWeight(upload) <= room ? upload : 'overBudget';
}
