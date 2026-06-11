import type { AiBookConfig } from '../types'

export const DEFAULT_AI_BOOK_CONFIG: AiBookConfig = {
  modelSource: 'browser',
  textBaseUrl: '',
  textApiKey: '',
  textModel: 'gpt-4o-mini',
  textUseFullUrl: false,
  imageBaseUrl: '',
  imageApiKey: '',
  imageModel: 'gpt-image-1',
  imageSize: '1024x1024',
  imageUseFullUrl: false,
  useBackendProxy: false,
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
      textUseFullUrl: Boolean(parsed.textUseFullUrl),
      imageBaseUrl: normalizeBaseUrl(parsed.imageBaseUrl || legacyBaseUrl || DEFAULT_AI_BOOK_CONFIG.imageBaseUrl),
      imageApiKey: (parsed.imageApiKey || legacyApiKey || DEFAULT_AI_BOOK_CONFIG.imageApiKey).trim(),
      imageModel: (parsed.imageModel || DEFAULT_AI_BOOK_CONFIG.imageModel).trim(),
      imageSize: (parsed.imageSize || DEFAULT_AI_BOOK_CONFIG.imageSize).trim(),
      imageUseFullUrl: Boolean(parsed.imageUseFullUrl),
      useBackendProxy: Boolean(parsed.useBackendProxy),
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
    textUseFullUrl: Boolean(config.textUseFullUrl),
    imageBaseUrl: normalizeBaseUrl(config.imageBaseUrl),
    imageApiKey: config.imageApiKey.trim(),
    imageModel: config.imageModel.trim(),
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
  currentOrigin = globalThis.location?.origin || '',
) {
  if (!canUseServerModel || config.modelSource === 'server') return false
  if (!config.textBaseUrl.trim()) return true
  return pointsToCurrentApp(config.textBaseUrl, currentOrigin)
}

export function aiBookConfigStorageKey(username?: string | null) {
  return storageKey(username)
}

function pointsToCurrentApp(baseUrl: string, currentOrigin: string) {
  try {
    const target = new URL(normalizeBaseUrl(baseUrl))
    const current = new URL(currentOrigin)
    if (target.origin === current.origin) return true
    return isLoopbackHost(target.hostname)
      && isLoopbackHost(current.hostname)
      && target.port === current.port
      && target.protocol === current.protocol
  } catch {
    return false
  }
}

function isLoopbackHost(hostname: string) {
  return hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '[::1]' || hostname === '::1'
}
