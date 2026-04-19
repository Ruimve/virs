import { type Component, createSignal, Show, For } from 'solid-js'
import { A, useLocation } from '@solidjs/router'
import { getUser, isAdmin, initAuth } from '../lib/auth'
import { logout } from '../lib/api'
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
  '/users': '用户管理',
}

// SVG 图标组件
function NavIcon(props: { name: string; class?: string }) {
  const iconClass = props.class || 'w-5 h-5'
  switch (props.name) {
    case 'dashboard':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
        </svg>
      )
    case 'strategies':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
        </svg>
      )
    case 'market':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
        </svg>
      )
    case 'backtest':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      )
    case 'trades':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
        </svg>
      )
    case 'credentials':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
        </svg>
      )
    case 'users':
      return (
        <svg class={iconClass} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
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

  // 初始化认证状态
  initAuth()

  const currentPageTitle = () => pageTitles[location.pathname] || 'VIRS'

  const handleLogout = () => {
    logout()
  }

  const filteredNavItems = () =>
    navItems.filter((item) => !item.adminOnly || isAdmin())

  return (
    <div class="flex h-screen bg-gray-100">
      {/* 移动端遮罩 */}
      <Show when={sidebarOpen()}>
        <div
          class="fixed inset-0 z-20 bg-black/50 lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      </Show>

      {/* 侧边栏 */}
      <aside
        class={`fixed inset-y-0 left-0 z-30 w-64 bg-gray-900 transition-transform duration-300 lg:translate-x-0 lg:static lg:z-auto ${
          sidebarOpen() ? 'translate-x-0' : '-translate-x-full'
        }`}
      >
        {/* Logo */}
        <div class="flex items-center h-16 px-6 bg-gray-800">
          <span class="text-xl font-bold text-white tracking-wide">VIRS</span>
        </div>

        {/* 导航菜单 */}
        <nav class="mt-6 px-3">
          <For each={filteredNavItems()}>
            {(item) => (
              <A
                href={item.path}
                onClick={() => setSidebarOpen(false)}
                class="flex items-center gap-3 px-3 py-2.5 mb-1 rounded-lg text-sm font-medium transition-colors duration-150"
                activeClass="bg-blue-600 text-white"
                inactiveClass="text-gray-300 hover:bg-gray-800 hover:text-white"
                end={item.path === '/dashboard'}
              >
                <NavIcon name={item.icon} />
                <span>{item.label}</span>
              </A>
            )}
          </For>
        </nav>

        {/* 底部退出按钮 */}
        <div class="absolute bottom-0 left-0 right-0 p-4 border-t border-gray-800">
          <button
            onClick={handleLogout}
            class="flex items-center gap-3 w-full px-3 py-2.5 rounded-lg text-sm font-medium text-gray-300 hover:bg-gray-800 hover:text-white transition-colors duration-150"
          >
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
            </svg>
            <span>退出登录</span>
          </button>
        </div>
      </aside>

      {/* 主内容区 */}
      <div class="flex-1 flex flex-col overflow-hidden">
        {/* 顶栏 */}
        <header class="flex items-center justify-between h-16 px-6 bg-white border-b border-gray-200 shadow-sm">
          {/* 左侧: 汉堡菜单 + 页面标题 */}
          <div class="flex items-center gap-4">
            <button
              class="lg:hidden p-2 rounded-lg text-gray-500 hover:bg-gray-100 hover:text-gray-700"
              onClick={() => setSidebarOpen(!sidebarOpen())}
            >
              <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
            <h1 class="text-lg font-semibold text-gray-800">{currentPageTitle()}</h1>
          </div>

          {/* 右侧: 用户名 + 退出 */}
          <div class="flex items-center gap-4">
            <Show when={getUser()}>
              {(currentUser) => (
                <div class="flex items-center gap-2">
                  <div class="w-8 h-8 rounded-full bg-blue-600 flex items-center justify-center text-white text-sm font-medium">
                    {currentUser().username.charAt(0).toUpperCase()}
                  </div>
                  <span class="text-sm font-medium text-gray-700 hidden sm:inline">
                    {currentUser().username}
                  </span>
                </div>
              )}
            </Show>
            <button
              onClick={handleLogout}
              class="p-2 rounded-lg text-gray-500 hover:bg-gray-100 hover:text-gray-700 transition-colors"
              title="退出登录"
            >
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
              </svg>
            </button>
          </div>
        </header>

        {/* 页面内容 */}
        <main class="flex-1 overflow-auto p-6">
          {props.children}
        </main>
      </div>
    </div>
  )
}

export default Layout
