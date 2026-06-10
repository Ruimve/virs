// Re-export everything for backward compatibility
export { api, getToken, setToken, removeToken } from './client'
export { login, logout, getUserInfo } from './auth'
export { fetchPlugins, validateScript, getAiStatus, generateStrategy } from './ai'
export { saveCredential, saveAiCredential, testCredential, checkPermissions, verifyPermissions, fetchAccountInfo, fetchCredentialStatus } from './credentials'
export { createGridBot, startGridBot, createAutoBot, startAutoBot, findActiveBot } from './bot'
export type * from './types'
