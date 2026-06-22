import { describe, expect, it, beforeEach } from 'vitest'
import {
  DEFAULT_AI_BOOK_CONFIG,
  aiBookConfigStorageKey,
  getAiBookConfig,
  isAiBookImageConfigReady,
  shouldAutoUseServerAiBookConfig,
  isAiBookConfigReady,
  mediaPresetFromPath,
  saveAiBookConfig,
  describeAiBookTextRuntime,
  textPathForPreset,
  textPresetFromPath,
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
      textPath: 'v1/responses',
      textUseFullUrl: true,
      imageBaseUrl: 'https://image.example.test/',
      imageApiKey: 'alice-image-key',
      imageModel: 'story-image',
      imagePath: '/v1/images/generations',
      imageSize: '1024x1024',
      imageUseFullUrl: false,
      useBackendProxy: true,
    })
    saveAiBookConfig('bob', {
      modelSource: 'browser',
      textBaseUrl: 'https://other-text.example.test',
      textApiKey: 'bob-text-key',
      textModel: 'other-text',
      textPath: '/v1beta/openai/chat/completions',
      textUseFullUrl: false,
      imageBaseUrl: 'https://other-image.example.test',
      imageApiKey: 'bob-image-key',
      imageModel: 'other-image',
      imagePath: '/v1/images/generations',
      imageSize: '1792x1024',
      imageUseFullUrl: true,
      useBackendProxy: false,
    })

    expect(getAiBookConfig('alice')).toMatchObject({
      textBaseUrl: 'https://text.example.test',
      textApiKey: 'alice-text-key',
      textModel: 'story-text',
      textPath: '/v1/responses',
      textUseFullUrl: true,
      imageBaseUrl: 'https://image.example.test',
      imageApiKey: 'alice-image-key',
      imageModel: 'story-image',
      imagePath: '/v1/images/generations',
      imageSize: '1024x1024',
      imageUseFullUrl: false,
      useBackendProxy: true,
    })
    expect(getAiBookConfig('bob').textApiKey).toBe('bob-text-key')
    expect(getAiBookConfig('bob').imageApiKey).toBe('bob-image-key')
  })

  it('falls back to defaults and reports readiness', () => {
    expect(getAiBookConfig('guest')).toEqual({ ...DEFAULT_AI_BOOK_CONFIG, useBackendProxy: true })
    expect(isAiBookConfigReady(getAiBookConfig('guest'))).toBe(false)
    expect(isAiBookImageConfigReady(getAiBookConfig('guest'))).toBe(false)

    saveAiBookConfig('guest', {
      modelSource: 'browser',
      textBaseUrl: 'http://localhost:8825',
      textApiKey: '',
      textModel: 'gpt-4o-mini',
      textPath: '/v1/chat/completions',
      textUseFullUrl: false,
      imageBaseUrl: '',
      imageApiKey: '',
      imageModel: 'gpt-image-1',
      imagePath: '/v1/images/generations',
      imageSize: '1024x1024',
      imageUseFullUrl: false,
      useBackendProxy: true,
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
      useBackendProxy: true,
    })
  })

  it('auto-prefers server config only when browser text config is empty', () => {
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
    )).toBe(false)
    expect(shouldAutoUseServerAiBookConfig(
      { ...baseConfig, textBaseUrl: 'http://localhost:8081' },
      true,
      'http://127.0.0.1:8081',
    )).toBe(false)
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

  it('describes the actual text runtime used by AI book generation', () => {
    expect(describeAiBookTextRuntime({
      ...DEFAULT_AI_BOOK_CONFIG,
      modelSource: 'browser',
      useBackendProxy: true,
      textModel: 'story-model',
      textPath: '/v1/responses',
    }, null)).toEqual({
      source: 'browser',
      sourceLabel: '浏览器配置，经后端代理',
      model: 'story-model',
      path: '/reader3/aiProxy → /v1/responses',
    })

    expect(describeAiBookTextRuntime({
      ...DEFAULT_AI_BOOK_CONFIG,
      modelSource: 'browser',
      useBackendProxy: false,
      textModel: 'direct-model',
      textPath: '/v1/chat/completions',
    }, null)).toMatchObject({
      sourceLabel: '浏览器直连',
      model: 'direct-model',
      path: '/v1/chat/completions',
    })

    expect(describeAiBookTextRuntime({
      ...DEFAULT_AI_BOOK_CONFIG,
      modelSource: 'server',
    }, {
      text: {
        enabled: true,
        baseUrl: 'https://api.example.test',
        apiKey: '',
        model: 'server-model',
        path: '/v1/chat/completions',
        useFullUrl: false,
      },
      image: {
        enabled: false,
        baseUrl: '',
        apiKey: '',
        model: '',
        path: '/v1/images/generations',
        imageSize: '1024x1024',
        useFullUrl: false,
      },
      speech: {
        enabled: false,
        baseUrl: '',
        apiKey: '',
        model: '',
        path: '/v1/audio/speech',
        voice: 'alloy',
        responseFormat: 'mp3',
        useFullUrl: false,
      },
    })).toEqual({
      source: 'server',
      sourceLabel: '后端配置',
      model: 'server-model',
      path: '/reader3/aiProxy → /v1/chat/completions',
    })
  })

  it('maps text provider presets to existing path values', () => {
    expect(textPathForPreset('chat', 'gpt-4o-mini')).toBe('/v1/chat/completions')
    expect(textPathForPreset('responses', 'gpt-5.5')).toBe('/v1/responses')
    expect(textPathForPreset('gemini', 'gemini-2.5-pro')).toBe('/v1beta/models/gemini-2.5-pro:generateContent')
    expect(textPathForPreset('anthropic', 'claude-sonnet-4')).toBe('/v1/messages')
  })

  it('infers provider presets from saved paths', () => {
    expect(textPresetFromPath('/v1/chat/completions')).toBe('chat')
    expect(textPresetFromPath('/v1/responses')).toBe('responses')
    expect(textPresetFromPath('/v1beta/models/gemini-2.5-pro:generateContent')).toBe('gemini')
    expect(textPresetFromPath('/v1/messages')).toBe('anthropic')
    expect(textPresetFromPath('/custom/path')).toBe('custom')
    expect(mediaPresetFromPath('/v1/images/generations', 'image')).toBe('openai-image')
    expect(mediaPresetFromPath('/v1/audio/speech', 'speech')).toBe('openai-speech')
    expect(mediaPresetFromPath('/custom/image', 'image')).toBe('custom')
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
