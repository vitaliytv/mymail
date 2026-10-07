import { describe, it, expect, vi, beforeEach } from 'vitest'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args) => invokeMock(...args) }))

const { normalizeBaseUrl, useLlm } = await import('./llm.js')

beforeEach(() => {
  invokeMock.mockReset()
  localStorage.clear()
})

describe('useLlm', () => {
  it('uses the one local-openai provider when nothing is stored', () => {
    const { provider, model } = useLlm({ storagePrefix: 'test', defaultModel: 'gemma' })
    expect(provider.value).toBe('local-openai')
    expect(model.value).toBe('gemma')
  })

  it('save() persists provider/model under the storagePrefix, reloaded by a later useLlm()', () => {
    const first = useLlm({ storagePrefix: 'test' })
    first.baseUrl.value = 'http://127.0.0.1:8080/v1'
    first.model.value = 'gemma-4-26b-a4b-it'
    first.save()

    const second = useLlm({ storagePrefix: 'test' })
    expect(second.provider.value).toBe('local-openai')
    expect(second.baseUrl.value).toBe('http://127.0.0.1:8080/v1/')
    expect(second.model.value).toBe('gemma-4-26b-a4b-it')
  })

  it('loads the launch-time URL and auto-resolves an empty model', async () => {
    invokeMock
      .mockResolvedValueOnce({ baseUrl: 'http://127.0.0.1:8080/v1/' })
      .mockResolvedValueOnce(['gemma-4-e4b', 'gemma-4-26b'])
    const { baseUrl, model, loadEnv } = useLlm({ storagePrefix: 'test' })
    await loadEnv()
    expect(baseUrl.value).toBe('http://127.0.0.1:8080/v1/')
    expect(invokeMock).toHaveBeenLastCalledWith('llm_list_models', {
      baseUrl: 'http://127.0.0.1:8080/v1/',
      apiKey: null
    })
    expect(model.value).toBe('gemma-4-e4b')
  })

  it('loadEnv() leaves a configured model untouched but still loads its endpoint', async () => {
    invokeMock.mockResolvedValue({ baseUrl: 'http://127.0.0.1:8080/v1/' })
    const { loadEnv } = useLlm({ storagePrefix: 'test', defaultModel: 'gemma' })
    await loadEnv()
    expect(invokeMock).toHaveBeenCalledTimes(1)
  })

  it('requires explicit configuration when no saved or launch-time URL exists', async () => {
    invokeMock.mockResolvedValue({ baseUrl: null })
    const { loadEnv } = useLlm({ storagePrefix: 'test' })
    await expect(loadEnv()).rejects.toThrow('LLM не налаштовано')
  })

  it('chat() invokes llm_chat with "provider/model" and returns { content }', async () => {
    invokeMock.mockResolvedValue('Привіт!')
    const { baseUrl, chat } = useLlm({ storagePrefix: 'test', defaultModel: 'gemma-4-26b-a4b-it' })
    baseUrl.value = 'http://127.0.0.1:8080/v1/'
    const reply = await chat({ system: 'sys', user: 'hi' })
    expect(invokeMock).toHaveBeenCalledWith('llm_chat', {
      modelSpec: 'local-openai/gemma-4-26b-a4b-it',
      system: 'sys',
      user: 'hi',
      baseUrl: 'http://127.0.0.1:8080/v1/',
      apiKey: null
    })
    expect(reply).toEqual({ content: 'Привіт!' })
  })

  it('chat() rejects with a timeout error when timeoutMs elapses before llm_chat resolves', async () => {
    invokeMock.mockImplementation(() => new Promise(resolve => setTimeout(() => resolve('late'), 50)))
    const { baseUrl, chat } = useLlm({ storagePrefix: 'test', defaultModel: 'gemma' })
    baseUrl.value = 'http://127.0.0.1:8080/v1/'
    await expect(chat({ user: 'hi', timeoutMs: 5 })).rejects.toThrow(/no response within/)
  })

  it('migrates the previous omlx keys into local-openai settings', () => {
    localStorage.setItem('test:llmProvider', 'omlx')
    localStorage.setItem('test:omlxBaseUrl', 'http://127.0.0.1:8080/v1/')
    localStorage.setItem('test:omlxModel', 'gemma')
    const { provider, baseUrl, model } = useLlm({ storagePrefix: 'test' })
    expect(provider.value).toBe('local-openai')
    expect(baseUrl.value).toBe('http://127.0.0.1:8080/v1/')
    expect(model.value).toBe('gemma')
  })

  it('canonicalizes only HTTP(S) /v1 API roots', () => {
    expect(normalizeBaseUrl('http://127.0.0.1:8080/v1')).toBe('http://127.0.0.1:8080/v1/')
    expect(() => normalizeBaseUrl('http://127.0.0.1:8080')).toThrow('/v1/')
  })
})
