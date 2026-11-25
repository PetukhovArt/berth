import { acpAttachment, type AcpAttachment, type AcpAttachmentUpload } from './berth';

export const MAX_UPLOAD_BYTES = 10 * 1024 * 1024;

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

function base64Of(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(',', 2)[1] ?? '');
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

export async function fileUpload(file: File): Promise<AcpAttachmentUpload | null> {
  if (file.type.startsWith('image/')) return { Image: { mime: file.type, name: file.name, data: await base64Of(file) } };
  const text = await file.text();
  return text.includes('\u0000') ? null : { File: { mime: file.type || 'text/plain', name: file.name, text } };
}
