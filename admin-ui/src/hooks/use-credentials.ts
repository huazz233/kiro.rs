import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  getCredentials,
  deleteCredential,
  setCredentialDisabled,
  setCredentialPriority,
  resetCredentialFailure,
  getCredentialBalance,
  getCachedBalances,
  getCredentialAccountInfo,
  addCredential,
  getCredentialStats,
  resetCredentialStats,
  resetAllStats,
  importTokenJson,
  getLoadBalancingMode,
  setLoadBalancingMode,
} from '@/api/credentials'
import type { AddCredentialRequest, ImportTokenJsonRequest } from '@/types/api'

// 查询凭据列表
export function useCredentials() {
  return useQuery({
    queryKey: ['credentials'],
    queryFn: getCredentials,
    refetchInterval: 30000, // 每 30 秒刷新一次
  })
}

// 查询凭据余额
export function useCredentialBalance(id: number | null) {
  return useQuery({
    queryKey: ['credential-balance', id],
    queryFn: () => getCredentialBalance(id!),
    enabled: id !== null,
    retry: false, // 余额查询失败时不重试（避免重复请求被封禁的账号）
  })
}

// 查询所有凭据的缓存余额（定时轮询，带退避策略）
export function useCachedBalances() {
  return useQuery({
    queryKey: ['cached-balances'],
    queryFn: getCachedBalances,
    refetchInterval: (query) => (query.state.error ? 60000 : 30000),
    refetchIntervalInBackground: false, // 页面不可见时暂停轮询
  })
}

// 查询凭据账号信息（套餐/用量/邮箱等）
export function useCredentialAccountInfo(id: number | null, enabled: boolean) {
  return useQuery({
    queryKey: ['credential-account', id],
    queryFn: () => getCredentialAccountInfo(id!),
    enabled: enabled && id !== null,
    retry: false,
  })
}

// 删除指定凭据
export function useDeleteCredential() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => deleteCredential(id),
    onSuccess: (_res, id) => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
      queryClient.invalidateQueries({ queryKey: ['credential-balance', id] })
      queryClient.invalidateQueries({ queryKey: ['credential-account', id] })
      queryClient.invalidateQueries({ queryKey: ['credential-stats', id] })
    },
  })
}

// 设置禁用状态
export function useSetDisabled() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, disabled }: { id: number; disabled: boolean }) =>
      setCredentialDisabled(id, disabled),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 设置优先级
export function useSetPriority() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, priority }: { id: number; priority: number }) =>
      setCredentialPriority(id, priority),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 重置失败计数
export function useResetFailure() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => resetCredentialFailure(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 添加新凭据
export function useAddCredential() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (req: AddCredentialRequest) => addCredential(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 查询指定凭据统计
export function useCredentialStats(id: number | null, enabled: boolean) {
  return useQuery({
    queryKey: ['credential-stats', id],
    queryFn: () => getCredentialStats(id!),
    enabled: enabled && id !== null,
    retry: false,
  })
}

// 清空指定凭据统计
export function useResetCredentialStats() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => resetCredentialStats(id),
    onSuccess: (_res, id) => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
      queryClient.invalidateQueries({ queryKey: ['credential-stats', id] })
    },
  })
}

// 清空全部统计
export function useResetAllStats() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => resetAllStats(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
      queryClient.invalidateQueries({ queryKey: ['credential-stats'] })
    },
  })
}

// 批量导入 token.json
export function useImportTokenJson() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (req: ImportTokenJsonRequest) => importTokenJson(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
      queryClient.invalidateQueries({ queryKey: ['cached-balances'] })
    },
  })
}

// 获取负载均衡模式
export function useLoadBalancingMode() {
  return useQuery({
    queryKey: ['loadBalancingMode'],
    queryFn: getLoadBalancingMode,
  })
}

// 设置负载均衡模式
export function useSetLoadBalancingMode() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: setLoadBalancingMode,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['loadBalancingMode'] })
    },
  })
}
