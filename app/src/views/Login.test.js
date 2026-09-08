import { describe, it, expect, vi, beforeEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { mountWithQuasar } from '../test-utils/quasar.js'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args) => invokeMock(...args) }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('test') }))
vi.mock('../composables/use-agent.js', () => ({ useAgent: () => ({}) }))

const { _resetForTest } = await import('../services/auth-store.js')
const loginModule = await import('./Login.vue')
const Login = loginModule.default

beforeEach(() => {
  invokeMock.mockReset()
  _resetForTest()
})

describe('Login.vue', () => {
  it('renders "Увійти через Google" when not authenticated', async () => {
    invokeMock.mockResolvedValue(false)
    const w = mountWithQuasar(Login)
    await flushPromises()
    expect(w.text()).toContain('Увійти через Google')
  })

  it('renders the authenticated inbox controls and a menu with "Вийти"', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('me@example.com')
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    expect(w.text()).toContain('Скринька порожня.')
    const menuBtn = w.findAll('button').find(b => b.html().includes('more_vert'))
    await menuBtn.trigger('click')
    await flushPromises()
    // q-menu content teleports to document.body, outside the wrapper's DOM tree.
    expect(document.body.textContent).toContain('Вийти')
  })

  it('shows Ukrainian error after failed login', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(false)
      if (cmd === 'auth_start_login') return Promise.reject(Object.assign(new Error('Network'), { kind: 'Network' }))
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    await w.find('button').trigger('click')
    await flushPromises()
    expect(w.text()).toContain("Не вдалося з'єднатися з Google. Перевірте мережу.")
  })

  it('calls auth_start_login when sign-in button clicked', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(false)
      if (cmd === 'auth_start_login') return Promise.resolve({ email: 'x@y' })
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    await w.find('button').trigger('click')
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledWith('auth_start_login')
  })

  it('calls auth_logout when sign-out button clicked', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('m@e')
      if (cmd === 'auth_logout') return Promise.resolve()
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    const menuBtn = w.findAll('button').find(b => b.html().includes('more_vert'))
    await menuBtn.trigger('click')
    await flushPromises()
    // q-menu content teleports to document.body, outside the wrapper's DOM tree.
    const logoutItem = [...document.body.querySelectorAll('.q-item')].find(el => el.textContent.includes('Вийти'))
    logoutItem.click()
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledWith('auth_logout')
    expect(w.text()).toContain('Увійти через Google')
  })
})

describe('Login.vue inbox count', () => {
  it('includes the inbox count in the window title after successful initialize', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return Promise.resolve(348)
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    expect(document.title).toBe('MyMail vtest - u@e - 348')
    w.unmount()
  })

  it('keeps the inbox view responsive while the count is loading', async () => {
    const { promise: pending, resolve: resolveCount } = Promise.withResolvers()
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return pending
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    expect(w.text()).toContain('Скринька порожня.')
    resolveCount(7)
    await flushPromises()
    expect(document.title).toBe('MyMail vtest - u@e - 7')
    w.unmount()
  })

  it('shows Ukrainian error when Gmail returns Http error', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return Promise.reject(Object.assign(new Error('Http'), { kind: 'Http' }))
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    expect(w.text()).toContain('Gmail повернув помилку. Спробуйте пізніше.')
  })
})

describe('Login.vue random message', () => {
  const sampleMessage = {
    id: 'm1',
    from: 'alice@example.com',
    subject: 'Greetings',
    date: 'Mon, 15 May 2026 10:00:00 +0300',
    body: 'hello body'
  }

  it('renders the message card after initialize', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return Promise.resolve(5)
      if (cmd === 'gmail_random_message') return Promise.resolve(sampleMessage)
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    expect(w.text()).toContain('alice@example.com')
    expect(w.text()).toContain('Greetings')
    expect(w.text()).toContain('Mon, 15 May 2026 10:00:00 +0300')
    expect(w.text()).toContain('hello body')
  })

  it('injects the link interceptor before a case-insensitive closing head tag', async () => {
    const htmlBody = '<html><head><title>Mail</title></HEAD><body>hello</body></html>'
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return Promise.resolve(1)
      if (cmd === 'gmail_random_message') return Promise.resolve({ ...sampleMessage, html_body: htmlBody })
      if (cmd === 'newsletter_template_list') return Promise.resolve([])
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login, { global: { stubs: { iframe: true } } })
    await flushPromises()
    const srcdoc = w.find('iframe').attributes('srcdoc')
    expect(srcdoc).toContain('<style>')
    expect(srcdoc).toContain('<script>')
    expect(srcdoc).toContain('</HEAD>')
    expect(srcdoc.indexOf('<style>')).toBeLessThan(srcdoc.indexOf('</HEAD>'))
    w.unmount()
  })

  it('shows "Скринька порожня." when Gmail returns Empty', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return Promise.resolve(0)
      if (cmd === 'gmail_random_message') return Promise.reject(Object.assign(new Error('Empty'), { kind: 'Empty' }))
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    expect(w.text()).toContain('Скринька порожня.')
  })

  it('clicking "Показати інший" re-invokes gmail_random_message', async () => {
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return Promise.resolve(5)
      if (cmd === 'gmail_random_message') return Promise.resolve(sampleMessage)
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    invokeMock.mockClear()
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'gmail_random_message') return Promise.resolve({ ...sampleMessage, id: 'm2', subject: 'Next one' })
      return Promise.resolve(null)
    })
    const btn = w.findAll('button').find(b => b.text().includes('Показати інший'))
    await btn.trigger('click')
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledWith('gmail_random_message', {})
    expect(w.text()).toContain('Next one')
  })

  it('toggleOnlyNewsletters calls setOnlyNewsletters and reloads message', async () => {
    const sampleNewsletter = { id: 'n1', from: 'nl@e', subject: 'NL', date: 'd', body: 'nl body' }
    invokeMock.mockImplementation(cmd => {
      if (cmd === 'auth_is_authenticated') return Promise.resolve(true)
      if (cmd === 'auth_current_email') return Promise.resolve('u@e')
      if (cmd === 'gmail_inbox_count') return Promise.resolve(1)
      if (cmd === 'gmail_random_message') return Promise.resolve({ ...sampleMessage, id: 'r1' })
      if (cmd === 'gmail_random_newsletter') return Promise.resolve(sampleNewsletter)
      return Promise.resolve(null)
    })
    const w = mountWithQuasar(Login)
    await flushPromises()
    const toggle = w.find('.q-toggle')
    await toggle.trigger('click')
    await flushPromises()
    expect(invokeMock).toHaveBeenCalledWith('gmail_random_newsletter', {})
  })
})
