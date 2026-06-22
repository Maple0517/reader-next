import type {
  AiBookConfig,
  AiServerModelConfig,
  ImageProviderPreset,
  SpeechProviderPreset,
  TextProviderPreset,
} from '../types'

export const DEFAULT_TEXT_MODEL_PATH = '/v1/chat/completions'
export const DEFAULT_IMAGE_MODEL_PATH = '/v1/images/generations'
export const DEFAULT_SPEECH_MODEL_PATH = '/v1/audio/speech'

export const DEFAULT_AI_BOOK_CONFIG: AiBookConfig = {
  modelSource: 'browser',
  textBaseUrl: '',
  textApiKey: '',
  textModel: 'gpt-4o-mini',
  textPath: DEFAULT_TEXT_MODEL_PATH,
  textUseFullUrl: false,
  imageBaseUrl: '',
  imageApiKey: '',
  imageModel: 'gpt-image-1',
  imagePath: DEFAULT_IMAGE_MODEL_PATH,
  imageSize: '1024x1024',
  imageUseFullUrl: false,
  useBackendProxy: true,
}

const AI_BOOK_CONFIG_PREFIX = 'reader-ai-book-config:'

function normalizeUsername(username?: string | null) {
  return (username || 'default').trim() || 'default'
}

function storageKey(username?: string | null) {
  return `${AI_BOOK_CONFIG_PREFIX}${normalizeUsername(username)}`
}

type LegacyAiBookConfig = Partial<AiBookConfig> & {
  baseUrl?: string
  apiKey?: string
}

function normalizeBaseUrl(url?: string | null) {
  return (url || '').trim().replace(/\/+$/, '')
}

function normalizeProxyPath(path: string | undefined, fallback: string) {
  const value = (path || '').trim() || fallback
  return value.startsWith('/') ? value : `/${value}`
}

export function textPathForPreset(preset: TextProviderPreset, model: string) {
  if (preset === 'responses') return '/v1/responses'
  if (preset === 'gemini') return `/v1beta/models/${encodeURIComponent(model.trim() || 'gemini-2.5-pro')}:generateContent`
  if (preset === 'anthropic') return '/v1/messages'
  return DEFAULT_TEXT_MODEL_PATH
}

export function textPresetFromPath(path: string): TextProviderPreset {
  const value = normalizeProxyPath(path, DEFAULT_TEXT_MODEL_PATH)
  if (value.endsWith(':generateContent') || value.endsWith(':streamGenerateContent')) return 'gemini'
  if (value === '/v1/responses') return 'responses'
  if (value === '/v1/messages') return 'anthropic'
  if (value.endsWith('/chat/completions')) return 'chat'
  return 'custom'
}

export function mediaPresetFromPath(path: string, kind: 'image' | 'speech'): ImageProviderPreset | SpeechProviderPreset {
  const fallback = kind === 'image' ? DEFAULT_IMAGE_MODEL_PATH : DEFAULT_SPEECH_MODEL_PATH
  const value = normalizeProxyPath(path, fallback)
  if (kind === 'image') return value.endsWith('/images/generations') ? 'openai-image' : 'custom'
  return value.endsWith('/audio/speech') ? 'openai-speech' : 'custom'
}

export function getAiBookConfig(username?: string | null): AiBookConfig {
  try {
    const raw = localStorage.getItem(storageKey(username))
    if (!raw) return { ...DEFAULT_AI_BOOK_CONFIG }
    const parsed = JSON.parse(raw) as LegacyAiBookConfig
    const legacyBaseUrl = normalizeBaseUrl(parsed.baseUrl)
    const legacyApiKey = (parsed.apiKey || '').trim()
    return {
      ...DEFAULT_AI_BOOK_CONFIG,
      modelSource: parsed.modelSource === 'server' ? 'server' : 'browser',
      textBaseUrl: normalizeBaseUrl(parsed.textBaseUrl || legacyBaseUrl || DEFAULT_AI_BOOK_CONFIG.textBaseUrl),
      textApiKey: (parsed.textApiKey || legacyApiKey || DEFAULT_AI_BOOK_CONFIG.textApiKey).trim(),
      textModel: (parsed.textModel || DEFAULT_AI_BOOK_CONFIG.textModel).trim(),
      textPath: normalizeProxyPath(parsed.textPath, DEFAULT_TEXT_MODEL_PATH),
      textUseFullUrl: Boolean(parsed.textUseFullUrl),
      imageBaseUrl: normalizeBaseUrl(parsed.imageBaseUrl || legacyBaseUrl || DEFAULT_AI_BOOK_CONFIG.imageBaseUrl),
      imageApiKey: (parsed.imageApiKey || legacyApiKey || DEFAULT_AI_BOOK_CONFIG.imageApiKey).trim(),
      imageModel: (parsed.imageModel || DEFAULT_AI_BOOK_CONFIG.imageModel).trim(),
      imagePath: normalizeProxyPath(parsed.imagePath, DEFAULT_IMAGE_MODEL_PATH),
      imageSize: (parsed.imageSize || DEFAULT_AI_BOOK_CONFIG.imageSize).trim(),
      imageUseFullUrl: Boolean(parsed.imageUseFullUrl),
      useBackendProxy: parsed.useBackendProxy == null ? DEFAULT_AI_BOOK_CONFIG.useBackendProxy : Boolean(parsed.useBackendProxy),
    }
  } catch {
    return { ...DEFAULT_AI_BOOK_CONFIG }
  }
}

export function saveAiBookConfig(username: string | null | undefined, config: AiBookConfig) {
  const next: AiBookConfig = {
    modelSource: config.modelSource === 'server' ? 'server' : 'browser',
    textBaseUrl: normalizeBaseUrl(config.textBaseUrl),
    textApiKey: config.textApiKey.trim(),
    textModel: config.textModel.trim(),
    textPath: normalizeProxyPath(config.textPath, DEFAULT_TEXT_MODEL_PATH),
    textUseFullUrl: Boolean(config.textUseFullUrl),
    imageBaseUrl: normalizeBaseUrl(config.imageBaseUrl),
    imageApiKey: config.imageApiKey.trim(),
    imageModel: config.imageModel.trim(),
    imagePath: normalizeProxyPath(config.imagePath, DEFAULT_IMAGE_MODEL_PATH),
    imageSize: config.imageSize.trim(),
    imageUseFullUrl: Boolean(config.imageUseFullUrl),
    useBackendProxy: Boolean(config.useBackendProxy),
  }
  localStorage.setItem(storageKey(username), JSON.stringify(next))
  return next
}

export function isAiBookConfigReady(config: AiBookConfig) {
  if (config.modelSource === 'server') return true
  return Boolean(config.textBaseUrl.trim() && config.textModel.trim())
}

export function isAiBookImageConfigReady(config: AiBookConfig) {
  if (config.modelSource === 'server') return true
  return Boolean(config.imageBaseUrl.trim() && config.imageModel.trim())
}

export function shouldAutoUseServerAiBookConfig(
  config: AiBookConfig,
  canUseServerModel: boolean,
  _currentOrigin = globalThis.location?.origin || '',
) {
  if (!canUseServerModel || config.modelSource === 'server') return false
  return !config.textBaseUrl.trim()
}

export interface AiBookTextRuntimeDescription {
  source: 'browser' | 'server'
  sourceLabel: string
  model: string
  path: string
}

export function describeAiBookTextRuntime(
  config: AiBookConfig,
  serverConfig: AiServerModelConfig | null | undefined,
): AiBookTextRuntimeDescription {
  if (config.modelSource === 'server') {
    const path = normalizeProxyPath(serverConfig?.text.path, DEFAULT_TEXT_MODEL_PATH)
    return {
      source: 'server',
      sourceLabel: '后端配置',
      model: (serverConfig?.text.model || '').trim() || '未配置',
      path: `/reader3/aiProxy → ${path}`,
    }
  }

  const path = normalizeProxyPath(config.textPath, DEFAULT_TEXT_MODEL_PATH)
  return {
    source: 'browser',
    sourceLabel: config.useBackendProxy ? '浏览器配置，经后端代理' : '浏览器直连',
    model: config.textModel.trim() || '未配置',
    path: config.useBackendProxy ? `/reader3/aiProxy → ${path}` : path,
  }
}

export function aiBookConfigStorageKey(username?: string | null) {
  return storageKey(username)
}
