import axios from 'axios'
import { buildAuthHeaderValues } from '../../utils/secureAccess'
import type { ApiResponse } from '../../types'

let lastNeedLoginDispatchAt = 0

function dispatchNeedLogin() {
  const now = Date.now()
  if (now - lastNeedLoginDispatchAt < 1500) return
  lastNeedLoginDispatchAt = now
  window.dispatchEvent(new CustomEvent('need-login'))
}

const v4Http = axios.create({
  baseURL: '/api/books/v4',
  timeout: 120000,
  headers: { 'Content-Type': 'application/json' },
})

v4Http.interceptors.request.use((config) => {
  const { accessToken, secureKey } = buildAuthHeaderValues(localStorage)
  if (accessToken) {
    config.headers.Authorization = accessToken
  }
  if (secureKey) {
    config.headers['X-Secure-Key'] = secureKey
  }
  return config
})

v4Http.interceptors.response.use(
  (response) => {
    const data = response.data as ApiResponse
    if (data.isSuccess === undefined) {
      return response
    }
    if (!data.isSuccess) {
      if (data.errorMsg === 'NEED_LOGIN' || data.data === 'NEED_LOGIN') {
        dispatchNeedLogin()
      }
      return Promise.reject(new Error(data.errorMsg || '请求失败'))
    }
    response.data = data.data
    return response
  },
  (error) => {
    const data = error.response?.data as Partial<ApiResponse> | undefined
    if (data && typeof data === 'object') {
      if (data.errorMsg === 'NEED_LOGIN' || data.data === 'NEED_LOGIN') {
        dispatchNeedLogin()
      }
      if (typeof data.errorMsg === 'string' && data.errorMsg.trim()) {
        return Promise.reject(new Error(data.errorMsg))
      }
    }
    if (error.response?.status === 401) {
      dispatchNeedLogin()
    }
    return Promise.reject(new Error(error.message || '请求失败'))
  },
)

export default v4Http
