import { describe, expect, it, beforeEach } from 'vitest'
import {
  DEFAULT_AI_BOOK_CONFIG,
  aiBookConfigStorageKey,
  getAiBookConfig,
  isAiBookImageConfigReady,
  shouldAutoUseServerAiBookConfig,
  isAiBookConfigReady,
  saveAiBookConfig,
} from './aiBookConfig'

beforeEach(() => {
  installLocalStorage()
  localStorage.clear()
})

describe('aiBookConfig', () => {
  it('persists model config by username', () => {
    saveAiBookConfig('alice', {
      modelSource: 'browser',
      textBaseUrl: 'https://text.example.test/',
      textApiKey: 'alice-text-key',
      textModel: 'story-text',
      textUseFullUrl: true,
      imageBaseUrl: 'https://image.example.test/',
      imageApiKey: 'alice-image-key',
      imageModel: 'story-image',
      imageSize: '1024x1024',
      imageUseFullUrl: false,
      useBackendProxy: true,
    })
    saveAiBookConfig('bob', {
      modelSource: 'browser',
      textBaseUrl: 'https://other-text.example.test',
      textApiKey: 'bob-text-key',
      textModel: 'other-text',
      textUseFullUrl: false,
      imageBaseUrl: 'https://other-image.example.test',
      imageApiKey: 'bob-image-key',
      imageModel: 'other-image',
      imageSize: '1792x1024',
      imageUseFullUrl: true,
      useBackendProxy: false,
    })

    expect(getAiBookConfig('alice')).toMatchObject({
      textBaseUrl: 'https://text.example.test',
      textApiKey: 'alice-text-key',
      textModel: 'story-text',
      textUseFullUrl: true,
      imageBaseUrl: 'https://image.example.test',
      imageApiKey: 'alice-image-key',
      imageModel: 'story-image',
      imageSize: '1024x1024',
      imageUseFullUrl: false,
      useBackendProxy: true,
    })
    expect(getAiBookConfig('bob').textApiKey).toBe('bob-text-key')
    expect(getAiBookConfig('bob').imageApiKey).toBe('bob-image-key')
  })

  it('falls back to defaults and reports readiness', () => {
    expect(getAiBookConfig('guest')).toEqual(DEFAULT_AI_BOOK_CONFIG)
    expect(isAiBookConfigReady(getAiBookConfig('guest'))).toBe(false)
    expect(isAiBookImageConfigReady(getAiBookConfig('guest'))).toBe(false)

    saveAiBookConfig('guest', {
      modelSource: 'browser',
      textBaseUrl: 'http://localhost:8825',
      textApiKey: '',
      textModel: 'gpt-4o-mini',
      textUseFullUrl: false,
      imageBaseUrl: '',
      imageApiKey: '',
      imageModel: 'gpt-image-1',
      imageSize: '1024x1024',
      imageUseFullUrl: false,
      useBackendProxy: false,
    })
    expect(isAiBookConfigReady(getAiBookConfig('guest'))).toBe(true)
    expect(isAiBookImageConfigReady(getAiBookConfig('guest'))).toBe(false)

    saveAiBookConfig('guest', {
      ...getAiBookConfig('guest'),
      modelSource: 'server',
      textBaseUrl: '',
      imageBaseUrl: '',
    })
    expect(isAiBookConfigReady(getAiBookConfig('guest'))).toBe(true)
    expect(isAiBookImageConfigReady(getAiBookConfig('guest'))).toBe(true)
  })

  it('migrates old shared endpoint config into separated text and image config', () => {
    localStorage.setItem(aiBookConfigStorageKey('legacy'), JSON.stringify({
      baseUrl: 'https://old.example.test/',
      apiKey: 'old-key',
      textModel: 'old-text',
      imageModel: 'old-image',
      imageSize: '1024x1792',
    }))

    expect(getAiBookConfig('legacy')).toMatchObject({
      textBaseUrl: 'https://old.example.test',
      textApiKey: 'old-key',
      textModel: 'old-text',
      textUseFullUrl: false,
      imageBaseUrl: 'https://old.example.test',
      imageApiKey: 'old-key',
      imageModel: 'old-image',
      imageSize: '1024x1792',
      imageUseFullUrl: false,
      useBackendProxy: false,
    })
  })

  it('auto-prefers server config when browser config would call the reader app itself', () => {
    const baseConfig = {
      ...DEFAULT_AI_BOOK_CONFIG,
      modelSource: 'browser' as const,
      textModel: 'gpt-5.5',
    }

    expect(shouldAutoUseServerAiBookConfig(
      { ...baseConfig, textBaseUrl: '' },
      true,
      'http://127.0.0.1:8081',
    )).toBe(true)
    expect(shouldAutoUseServerAiBookConfig(
      { ...baseConfig, textBaseUrl: 'http://127.0.0.1:8081' },
      true,
      'http://127.0.0.1:8081',
    )).toBe(true)
    expect(shouldAutoUseServerAiBookConfig(
      { ...baseConfig, textBaseUrl: 'http://localhost:8081' },
      true,
      'http://127.0.0.1:8081',
    )).toBe(true)
    expect(shouldAutoUseServerAiBookConfig(
      { ...baseConfig, textBaseUrl: 'http://127.0.0.1:8080' },
      true,
      'http://127.0.0.1:8081',
    )).toBe(false)
    expect(shouldAutoUseServerAiBookConfig(
      { ...baseConfig, textBaseUrl: 'http://127.0.0.1:8081' },
      false,
      'http://127.0.0.1:8081',
    )).toBe(false)
  })
})

function installLocalStorage() {
  const memory = new Map<string, string>()
  Object.defineProperty(globalThis, 'localStorage', {
    value: {
      getItem: (key: string) => memory.get(key) || null,
      setItem: (key: string, value: string) => memory.set(key, value),
      removeItem: (key: string) => memory.delete(key),
      clear: () => memory.clear(),
    },
    configurable: true,
  })
}
