import { type Component, createSignal, Show, For } from 'solid-js'
import { A, useLocation } from '@solidjs/router'
import { getUser, isAdmin, initAuth } from '../lib/auth'
import { logout } from '../lib/api'
import { useMarket } from '../lib/market-context'
import type { RouteSectionProps } from '@solidjs/router'

interface NavItem {
  path: string
  label: string
  icon: string
  adminOnly?: boolean
}

const navItems: NavItem[] = [
  { path: '/dashboard', label: '仪表盘', icon: 'dashboard' },
  { path: '/strategies', label: '策略管理', icon: 'strategies' },
  { path: '/market', label: '行情查看', icon: 'market' },
  { path: '/backtest', label: '回测', icon: 'backtest' },
  { path: '/trades', label: '交易记录', icon: 'trades' },
  { path: '/credentials', label: '凭证管理', icon: 'credentials' },
  { path: '/ai-credentials', label: 'AI 凭证', icon: 'ai' },
  { path: '/users', label: '用户管理', icon: 'users', adminOnly: true },
]

// 页面标题映射
const pageTitles: Record<string, string> = {
  '/dashboard': '仪表盘',
  '/strategies': '策略管理',
  '/market': '行情查看',
  '/backtest': '回测',
  '/trades': '交易记录',
  '/credentials': '凭证管理',
  '/ai-credentials': 'AI 凭证',
  '/users': '用户管理',
}

// SVG 图标组件
function NavIcon(props: { name: string; class?: string }) {
  const iconClass = props.class || 'w-[18px] h-[18px]'
  switch (props.name) {
    case 'dashboard':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
        </svg>
      )
    case 'strategies':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
        </svg>
      )
    case 'market':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
        </svg>
      )
    case 'backtest':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      )
    case 'trades':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
        </svg>
      )
    case 'credentials':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
        </svg>
      )
    case 'ai':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 00-2.455 2.456zM16.894 20.567L16.5 21.75l-.394-1.183a2.25 2.25 0 00-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 001.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 001.423 1.423l1.183.394-1.183.394a2.25 2.25 0 00-1.423 1.423z" />
        </svg>
      )
    case 'users':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
        </svg>
      )
    default:
      return null
  }
}

const Layout: Component<RouteSectionProps> = (props) => {
  const location = useLocation()
  const [sidebarOpen, setSidebarOpen] = createSignal(false)
  const market = useMarket()

  // 初始化认证状态
  initAuth()

  const currentPageTitle = () => pageTitles[location.pathname] || 'VIRS'

  const handleLogout = () => {
    logout()
  }

  const filteredNavItems = () =>
    navItems.filter((item) => !item.adminOnly || isAdmin())

  // Login 页面不渲染侧边栏和顶栏
  const isLoginPage = () => location.pathname === '/login'

  return (
    <Show when={!isLoginPage()} fallback={props.children}>
      <div class="flex h-screen bg-[var(--color-bg-secondary)]">
      {/* 移动端遮罩 */}
      <Show when={sidebarOpen()}>
        <div
          class="fixed inset-0 z-20 bg-black/20 backdrop-blur-sm lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      </Show>

      {/* 侧边栏 */}
      <aside
        class={`fixed inset-y-0 left-0 z-30 w-60 bg-white border-r border-[var(--color-border)] transition-transform duration-300 ease-in-out lg:translate-x-0 lg:static lg:z-auto ${
          sidebarOpen() ? 'translate-x-0' : '-translate-x-full'
        }`}
      >
        {/* Logo */}
        <div class="flex items-center h-16 px-6">
          <span class="text-lg font-semibold text-[var(--color-text-primary)] tracking-[0.15em]">VIRS</span>
        </div>

        {/* 导航菜单 */}
        <nav class="mt-2 px-3">
          <For each={filteredNavItems()}>
            {(item) => (
              <A
                href={item.path}
                onClick={() => setSidebarOpen(false)}
                class="group relative flex items-center gap-3 px-3 py-2 mb-0.5 rounded-lg text-[13px] font-medium transition-all duration-200"
                activeClass="bg-[var(--color-accent-light)] text-[var(--color-accent)] [&::before]:content-[''] [&::before]:absolute [&::before]:left-0 [&::before]:top-1/2 [&::before]:-translate-y-1/2 [&::before]:w-[2px] [&::before]:h-4 [&::before]:rounded-full [&::before]:bg-[var(--color-accent)]"
                inactiveClass="text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
                end={item.path === '/dashboard'}
              >
                <NavIcon name={item.icon} class="w-4 h-4 opacity-60 group-[.group]:opacity-100 transition-opacity" />
                <span>{item.label}</span>
              </A>
            )}
          </For>
        </nav>

        {/* 底部退出按钮 */}
        <div class="absolute bottom-0 left-0 right-0 p-3 border-t border-[var(--color-border-light)]">
          <button
            onClick={handleLogout}
            class="flex items-center gap-3 w-full px-3 py-2 rounded-lg text-[13px] font-medium text-[var(--color-text-tertiary)] hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-secondary)] transition-all duration-200"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
              <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
            </svg>
            <span>退出登录</span>
          </button>
        </div>
      </aside>

      {/* 主内容区 */}
      <div class="flex-1 flex flex-col overflow-hidden">
        {/* 顶栏 */}
        <header class="flex items-center justify-between h-16 px-6 bg-white border-b border-[var(--color-border)]">
          {/* 左侧: 汉堡菜单 + 页面标题 */}
          <div class="flex items-center gap-4">
            <button
              class="lg:hidden p-2 rounded-lg text-[var(--color-text-tertiary)] hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-secondary)] transition-colors duration-200"
              onClick={() => setSidebarOpen(!sidebarOpen())}
            >
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
            <h1 class="text-[15px] font-semibold text-[var(--color-text-primary)]">{currentPageTitle()}</h1>
          </div>

          {/* 右侧: 市场类型开关 + 用户名 + 退出 */}
          <div class="flex items-center gap-3">
            {/* 全局市场类型开关 */}
            <div class="flex items-center bg-gray-100 rounded-lg p-0.5">
              <button
                class={`px-2.5 py-1 rounded-md text-[12px] font-medium transition-all duration-200 ${
                  market.marketType() === 'perpetual'
                    ? 'bg-white text-indigo-600 shadow-sm'
                    : 'text-gray-400 hover:text-gray-600'
                }`}
                onClick={() => market.setMarketType('perpetual')}
              >
                永续
              </button>
              <button
                class={`px-2.5 py-1 rounded-md text-[12px] font-medium transition-all duration-200 ${
                  market.marketType() === 'spot'
                    ? 'bg-white text-indigo-600 shadow-sm'
                    : 'text-gray-400 hover:text-gray-600'
                }`}
                onClick={() => market.setMarketType('spot')}
              >
                现货
              </button>
            </div>

            <Show when={getUser()}>
              {(currentUser) => (
                <div class="flex items-center gap-2.5">
                  <div class="w-8 h-8 rounded-lg bg-[var(--color-accent)] flex items-center justify-center text-white text-xs font-medium">
                    {currentUser().username.charAt(0).toUpperCase()}
                  </div>
                  <span class="text-[13px] font-medium text-[var(--color-text-secondary)] hidden sm:inline">
                    {currentUser().username}
                  </span>
                </div>
              )}
            </Show>
            <button
              onClick={handleLogout}
              class="p-2 rounded-lg text-[var(--color-text-tertiary)] hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-secondary)] transition-colors duration-200"
              title="退出登录"
            >
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
              </svg>
            </button>
          </div>
        </header>

        {/* 页面内容 */}
        <main class="flex-1 overflow-auto p-6 animate-fade-in">
          {props.children}
        </main>
      </div>
    </div>
    </Show>
  )
}

export default Layout
