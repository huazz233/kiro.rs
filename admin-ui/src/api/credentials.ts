import axios from 'axios'
import { storage } from '@/lib/storage'
import type {
  CredentialsStatusResponse,
  BalanceResponse,
  CachedBalancesResponse,
  SuccessResponse,
  SetDisabledRequest,
  SetPriorityRequest,
  AddCredentialRequest,
  AddCredentialResponse,
  CredentialStatsResponse,
  CredentialAccountInfoResponse,
  ImportTokenJsonRequest,
  ImportTokenJsonResponse,
} from '@/types/api'

// 创建 axios 实例
const api = axios.create({
  baseURL: '/api/admin',
  headers: {
    'Content-Type': 'application/json',
  },
})

// 请求拦截器添加 API Key
api.interceptors.request.use((config) => {
  const apiKey = storage.getApiKey()
  if (apiKey) {
    config.headers['x-api-key'] = apiKey
  }
  return config
})

// 获取所有凭据状态
export async function getCredentials(): Promise<CredentialsStatusResponse> {
  const { data } = await api.get<CredentialsStatusResponse>('/credentials')
  return data
}

// 设置凭据禁用状态
export async function setCredentialDisabled(
  id: number,
  disabled: boolean
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(
    `/credentials/${id}/disabled`,
    { disabled } as SetDisabledRequest
  )
  return data
}

// 设置凭据优先级
export async function setCredentialPriority(
  id: number,
  priority: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(
    `/credentials/${id}/priority`,
    { priority } as SetPriorityRequest
  )
  return data
}

// 重置失败计数
export async function resetCredentialFailure(
  id: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/credentials/${id}/reset`)
  return data
}

// 获取凭据余额
export async function getCredentialBalance(id: number): Promise<BalanceResponse> {
  const { data } = await api.get<BalanceResponse>(`/credentials/${id}/balance`)
  return data
}

// 获取所有凭据的缓存余额
export async function getCachedBalances(): Promise<CachedBalancesResponse> {
  const { data } = await api.get<CachedBalancesResponse>('/credentials/balances/cached')
  return data
}

// 获取凭据账号信息（套餐/用量/邮箱等）
export async function getCredentialAccountInfo(
  id: number
): Promise<CredentialAccountInfoResponse> {
  const { data } = await api.get<CredentialAccountInfoResponse>(`/credentials/${id}/account`)
  return data
}

// 添加新凭据
export async function addCredential(
  req: AddCredentialRequest
): Promise<AddCredentialResponse> {
  const { data } = await api.post<AddCredentialResponse>('/credentials', req)
  return data
}

// 删除凭据
export async function deleteCredential(id: number): Promise<SuccessResponse> {
  const { data } = await api.delete<SuccessResponse>(`/credentials/${id}`)
  return data
}

// 获取指定凭据统计
export async function getCredentialStats(id: number): Promise<CredentialStatsResponse> {
  const { data } = await api.get<CredentialStatsResponse>(`/credentials/${id}/stats`)
  return data
}

// 清空指定凭据统计
export async function resetCredentialStats(id: number): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/credentials/${id}/stats/reset`)
  return data
}

// 清空全部统计
export async function resetAllStats(): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>('/stats/reset')
  return data
}

// 批量导入 token.json
export async function importTokenJson(
  req: ImportTokenJsonRequest
): Promise<ImportTokenJsonResponse> {
  const { data } = await api.post<ImportTokenJsonResponse>(
    '/credentials/import-token-json',
    req
  )
  return data
}

// 获取负载均衡模式
export async function getLoadBalancingMode(): Promise<{ mode: 'priority' | 'balanced' }> {
  const { data } = await api.get<{ mode: 'priority' | 'balanced' }>('/config/load-balancing')
  return data
}

// 设置负载均衡模式
export async function setLoadBalancingMode(mode: 'priority' | 'balanced'): Promise<{ mode: 'priority' | 'balanced' }> {
  const { data } = await api.put<{ mode: 'priority' | 'balanced' }>('/config/load-balancing', { mode })
  return data
}
