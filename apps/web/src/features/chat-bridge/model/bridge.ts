import { invoke } from '../../../bridge';

export type ChatBridge = { telegram_token: string; telegram_chat: string; telegram_owner: string; webhook_url: string };

export const readChatBridge = () => invoke<ChatBridge>('chat_bridge_read');

export const writeChatBridge = (bridge: ChatBridge) => invoke<void>('chat_bridge_write', { telegramToken: bridge.telegram_token, telegramChat: bridge.telegram_chat, telegramOwner: bridge.telegram_owner, webhookUrl: bridge.webhook_url });

export const sendChatBridgeTest = () => invoke<unknown>('berth_call', { request: { TestChatBridge: {} } });
