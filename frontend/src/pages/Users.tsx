import { type Component, createSignal, createEffect, Show, For } from 'solid-js'
import { api, type PaginatedResponse } from '../lib/api'
import { isAdmin } from '../lib/auth'

// ---- 类型定义 ----

interface User {
  id: string
  username: string
  role: 'admin' | 'manager' | 'user' | 'viewer'
  email?: string
  is_active: boolean
  credits: number
  created_at: string
}

interface CreateUserRequest {
  username: string
  password: string
  email?: string
  role?: string
  credits?: number
}

interface UpdateUserRequest {
  email?: string
  role?: string
  is_active?: boolean
  credits?: number
  password?: string
}

// ---- 角色配置 ----

const ROLE_CONFIG: Record<string, { label: string; color: string }> = {
  admin: { label: '管理员', color: 'bg-red-100 text-red-700' },
  manager: { label: '经理', color: 'bg-purple-100 text-purple-700' },
  user: { label: '用户', color: 'bg-blue-100 text-blue-700' },
  viewer: { label: '观察者', color: 'bg-gray-100 text-gray-700' },
}

function getRoleBadge(role: string) {
  return ROLE_CONFIG[role] || { label: role, color: 'bg-gray-100 text-gray-700' }
}

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr)
    return d.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
  } catch {
    return dateStr
  }
}

// ---- 组件 ----

const Users: Component = () => {
  const [users, setUsers] = createSignal<User[]>([])
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal<string | null>(null)

  // 分页
  const [page, setPage] = createSignal(1)
  const [pageSize] = createSignal(20)
  const [totalPages, setTotalPages] = createSignal(1)
  const [total, setTotal] = createSignal(0)

  // 模态框
  const [showModal, setShowModal] = createSignal(false)
  const [editingUser, setEditingUser] = createSignal<User | null>(null)
  const [saving, setSaving] = createSignal(false)
  const [formError, setFormError] = createSignal<string | null>(null)

  // 表单
  const [formUsername, setFormUsername] = createSignal('')
  const [formPassword, setFormPassword] = createSignal('')
  const [formEmail, setFormEmail] = createSignal('')
  const [formRole, setFormRole] = createSignal('user')
  const [formIsActive, setFormIsActive] = createSignal(true)
  const [formCredits, setFormCredits] = createSignal(0)

  // 获取用户列表
  async function fetchUsers() {
    setLoading(true)
    setError(null)
    try {
      const res = await api.get<PaginatedResponse<User>>(`/users/list?page=${page()}&page_size=${pageSize()}`)
      if (res.success && res.data) {
        setUsers(res.data.items)
        setTotal(res.data.total)
        setTotalPages(res.data.total_pages)
      } else {
        setError(res.error || '获取用户列表失败')
      }
    } catch (e: any) {
      setError(e.message || '网络错误')
    } finally {
      setLoading(false)
    }
  }

  // 翻页
  function goToPage(p: number) {
    if (p < 1 || p > totalPages()) return
    setPage(p)
  }

  // 翻页时重新加载
  const prevPage = () => goToPage(page() - 1)
  const nextPage = () => goToPage(page() + 1)

  // 打开创建模态框
  function openCreateModal() {
    setEditingUser(null)
    setFormUsername('')
    setFormPassword('')
    setFormEmail('')
    setFormRole('user')
    setFormIsActive(true)
    setFormCredits(0)
    setFormError(null)
    setShowModal(true)
  }

  // 打开编辑模态框
  function openEditModal(user: User) {
    setEditingUser(user)
    setFormUsername(user.username)
    setFormPassword('')
    setFormEmail(user.email || '')
    setFormRole(user.role)
    setFormIsActive(user.is_active)
    setFormCredits(user.credits)
    setFormError(null)
    setShowModal(true)
  }

  // 关闭模态框
  function closeModal() {
    setShowModal(false)
    setEditingUser(null)
    setFormError(null)
  }

  // 保存用户 (创建或更新)
  async function handleSave() {
    setSaving(true)
    setFormError(null)

    const isEdit = editingUser() !== null

    // 创建时校验必填
    if (!isEdit) {
      if (!formUsername().trim()) {
        setFormError('请输入用户名')
        setSaving(false)
        return
      }
      if (!formPassword().trim()) {
        setFormError('请输入密码')
        setSaving(false)
        return
      }
    }

    try {
      if (isEdit) {
        const req: UpdateUserRequest = {
          role: formRole(),
          is_active: formIsActive(),
          credits: formCredits(),
        }
        if (formEmail().trim()) {
          req.email = formEmail().trim()
        }
        if (formPassword().trim()) {
          req.password = formPassword().trim()
        }
        const res = await api.put(`/users/update/${editingUser()!.id}`, req)
        if (res.success) {
          closeModal()
          await fetchUsers()
        } else {
          setFormError(res.error || '更新失败')
        }
      } else {
        const req: CreateUserRequest = {
          username: formUsername().trim(),
          password: formPassword().trim(),
          role: formRole(),
          credits: formCredits(),
        }
        if (formEmail().trim()) {
          req.email = formEmail().trim()
        }
        const res = await api.post('/users/create', req)
        if (res.success) {
          closeModal()
          await fetchUsers()
        } else {
          setFormError(res.error || '创建失败')
        }
      }
    } catch (e: any) {
      setFormError(e.message || '网络错误')
    } finally {
      setSaving(false)
    }
  }

  // 删除用户
  async function handleDelete(id: string) {
    if (!window.confirm('确定要删除此用户吗？删除后不可恢复。')) return
    try {
      const res = await api.del(`/users/delete/${id}`)
      if (res.success) {
        await fetchUsers()
      } else {
        alert(res.error || '删除失败')
      }
    } catch (e: any) {
      alert(e.message || '网络错误')
    }
  }

  // ---- 权限检查 ----
  // 如果不是管理员，显示无权限提示（在 API 调用之前检查，避免无意义请求）
  if (!isAdmin()) {
    return (
      <div class="flex items-center justify-center py-20">
        <div class="text-center">
          <svg class="w-16 h-16 mx-auto text-gray-300 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0-10.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z" />
          </svg>
          <p class="text-gray-500 text-lg font-medium">无权限访问此页面</p>
          <p class="text-sm text-gray-400 mt-1">仅管理员可访问用户管理</p>
        </div>
      </div>
    )
  }

  // 初始化加载 + 翻页时重新加载
  let skipEffect = true
  fetchUsers().then(() => {
    skipEffect = false
  })
  createEffect(() => {
    // 读取 page() 以建立响应式依赖
    page()
    if (!skipEffect) {
      fetchUsers()
    }
  })

  // 分页页码数组
  function getPageNumbers(): number[] {
    const current = page()
    const last = totalPages()
    const pages: number[] = []
    const maxVisible = 5
    let start = Math.max(1, current - Math.floor(maxVisible / 2))
    let end = Math.min(last, start + maxVisible - 1)
    if (end - start < maxVisible - 1) {
      start = Math.max(1, end - maxVisible + 1)
    }
    for (let i = start; i <= end; i++) {
      pages.push(i)
    }
    return pages
  }

  return (
    <div class="space-y-6">
      {/* 标题栏 */}
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-semibold text-gray-800">用户管理</h2>
          <p class="text-sm text-gray-500 mt-1">管理系统用户和权限</p>
        </div>
        <button
          onClick={openCreateModal}
          class="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
        >
          创建用户
        </button>
      </div>

      {/* 加载中 */}
      <Show when={loading()}>
        <div class="flex items-center justify-center py-16">
          <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
          <span class="ml-3 text-gray-500">加载中...</span>
        </div>
      </Show>

      {/* 错误 */}
      <Show when={!loading() && error()}>
        <div class="bg-red-50 border border-red-200 rounded-xl p-6 text-center">
          <svg class="w-12 h-12 mx-auto text-red-400 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
          </svg>
          <p class="text-red-600">{error()}</p>
          <button
            onClick={fetchUsers}
            class="mt-3 px-4 py-1.5 text-sm bg-red-100 text-red-700 rounded-lg hover:bg-red-200 transition-colors"
          >
            重试
          </button>
        </div>
      </Show>

      {/* 用户表格 */}
      <Show when={!loading() && !error()}>
        <div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
          <Show
            when={users().length > 0}
            fallback={
              <div class="p-12 text-center">
                <svg class="w-16 h-16 mx-auto text-gray-300 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
                </svg>
                <p class="text-gray-500">暂无用户</p>
                <p class="text-sm text-gray-400 mt-1">点击"创建用户"添加第一个用户</p>
              </div>
            }
          >
            <div class="overflow-x-auto">
              <table class="w-full text-sm">
                <thead>
                  <tr class="border-b border-gray-200 bg-gray-50">
                    <th class="text-left px-4 py-3 font-medium text-gray-600">用户名</th>
                    <th class="text-left px-4 py-3 font-medium text-gray-600">角色</th>
                    <th class="text-left px-4 py-3 font-medium text-gray-600">邮箱</th>
                    <th class="text-left px-4 py-3 font-medium text-gray-600">状态</th>
                    <th class="text-left px-4 py-3 font-medium text-gray-600">积分</th>
                    <th class="text-left px-4 py-3 font-medium text-gray-600">创建时间</th>
                    <th class="text-right px-4 py-3 font-medium text-gray-600">操作</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={users()}>
                    {(user) => {
                      const roleBadge = getRoleBadge(user.role)
                      return (
                        <tr class="border-b border-gray-100 hover:bg-gray-50 transition-colors">
                          {/* 用户名 */}
                          <td class="px-4 py-3 font-medium text-gray-800">{user.username}</td>
                          {/* 角色 */}
                          <td class="px-4 py-3">
                            <span class={`inline-block px-2.5 py-0.5 rounded-full text-xs font-medium ${roleBadge.color}`}>
                              {roleBadge.label}
                            </span>
                          </td>
                          {/* 邮箱 */}
                          <td class="px-4 py-3 text-gray-500">{user.email || '-'}</td>
                          {/* 状态 */}
                          <td class="px-4 py-3">
                            <Show
                              when={user.is_active}
                              fallback={
                                <span class="inline-flex items-center gap-1.5 text-red-600">
                                  <span class="w-2 h-2 rounded-full bg-red-500"></span>
                                  禁用
                                </span>
                              }
                            >
                              <span class="inline-flex items-center gap-1.5 text-green-600">
                                <span class="w-2 h-2 rounded-full bg-green-500"></span>
                                活跃
                              </span>
                            </Show>
                          </td>
                          {/* 积分 */}
                          <td class="px-4 py-3 text-gray-600">{user.credits}</td>
                          {/* 创建时间 */}
                          <td class="px-4 py-3 text-gray-400">{formatDate(user.created_at)}</td>
                          {/* 操作 */}
                          <td class="px-4 py-3 text-right">
                            <div class="flex items-center justify-end gap-2">
                              <button
                                onClick={() => openEditModal(user)}
                                class="px-3 py-1.5 text-xs font-medium text-blue-600 bg-blue-50 rounded-lg hover:bg-blue-100 transition-colors"
                              >
                                编辑
                              </button>
                              <button
                                onClick={() => handleDelete(user.id)}
                                class="px-3 py-1.5 text-xs font-medium text-red-600 bg-red-50 rounded-lg hover:bg-red-100 transition-colors"
                              >
                                删除
                              </button>
                            </div>
                          </td>
                        </tr>
                      )
                    }}
                  </For>
                </tbody>
              </table>
            </div>

            {/* 分页 */}
            <Show when={totalPages() > 1}>
              <div class="flex items-center justify-between px-4 py-3 border-t border-gray-200 bg-gray-50">
                <p class="text-sm text-gray-500">
                  共 {total()} 条，第 {page()}/{totalPages()} 页
                </p>
                <div class="flex items-center gap-1">
                  {/* 上一页 */}
                  <button
                    onClick={prevPage}
                    disabled={page() <= 1}
                    class="px-3 py-1.5 text-sm rounded-lg border border-gray-300 text-gray-600 hover:bg-gray-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    上一页
                  </button>

                  {/* 页码 */}
                  <For each={getPageNumbers()}>
                    {(p) => (
                      <button
                        onClick={() => goToPage(p)}
                        class={`px-3 py-1.5 text-sm rounded-lg transition-colors ${
                          p === page()
                            ? 'bg-blue-600 text-white'
                            : 'border border-gray-300 text-gray-600 hover:bg-gray-100'
                        }`}
                      >
                        {p}
                      </button>
                    )}
                  </For>

                  {/* 下一页 */}
                  <button
                    onClick={nextPage}
                    disabled={page() >= totalPages()}
                    class="px-3 py-1.5 text-sm rounded-lg border border-gray-300 text-gray-600 hover:bg-gray-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    下一页
                  </button>
                </div>
              </div>
            </Show>
          </Show>
        </div>
      </Show>

      {/* 创建/编辑用户模态框 */}
      <Show when={showModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center">
          {/* 遮罩 */}
          <div class="absolute inset-0 bg-black/50" onClick={closeModal}></div>

          {/* 模态框内容 */}
          <div class="relative bg-white rounded-2xl shadow-xl w-full max-w-md mx-4 p-6 space-y-5">
            {/* 标题 */}
            <div class="flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-800">
                {editingUser() ? '编辑用户' : '创建用户'}
              </h3>
              <button
                onClick={closeModal}
                class="text-gray-400 hover:text-gray-600 transition-colors"
              >
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            {/* 表单 */}
            <div class="space-y-4">
              {/* 用户名 */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">用户名</label>
                <input
                  type="text"
                  value={formUsername()}
                  onInput={(e) => setFormUsername(e.currentTarget.value)}
                  disabled={editingUser() !== null}
                  placeholder="输入用户名"
                  class={`w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent ${
                    editingUser() ? 'bg-gray-100 text-gray-500 cursor-not-allowed' : 'bg-white'
                  }`}
                />
              </div>

              {/* 密码 */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">
                  密码
                  <Show when={editingUser() !== null}>
                    <span class="text-gray-400 font-normal ml-1">(留空则不修改)</span>
                  </Show>
                </label>
                <input
                  type="password"
                  value={formPassword()}
                  onInput={(e) => setFormPassword(e.currentTarget.value)}
                  placeholder={editingUser() ? '留空则不修改' : '输入密码'}
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white"
                />
              </div>

              {/* 邮箱 */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">
                  邮箱 <span class="text-gray-400 font-normal">(可选)</span>
                </label>
                <input
                  type="email"
                  value={formEmail()}
                  onInput={(e) => setFormEmail(e.currentTarget.value)}
                  placeholder="输入邮箱"
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white"
                />
              </div>

              {/* 角色 */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">角色</label>
                <select
                  value={formRole()}
                  onChange={(e) => setFormRole(e.currentTarget.value)}
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white"
                >
                  <option value="admin">管理员</option>
                  <option value="manager">经理</option>
                  <option value="user">用户</option>
                  <option value="viewer">观察者</option>
                </select>
              </div>

              {/* 状态 */}
              <div class="flex items-center gap-3">
                <label class="text-sm font-medium text-gray-700">状态</label>
                <label class="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={formIsActive()}
                    onChange={(e) => setFormIsActive(e.currentTarget.checked)}
                    class="sr-only peer"
                  />
                  <div class="w-9 h-5 bg-gray-200 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-green-500"></div>
                  <span class="ml-2 text-sm text-gray-600">
                    {formIsActive() ? '活跃' : '禁用'}
                  </span>
                </label>
              </div>

              {/* 积分 */}
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">积分</label>
                <input
                  type="number"
                  value={formCredits()}
                  onInput={(e) => setFormCredits(Number(e.currentTarget.value) || 0)}
                  min="0"
                  class="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white"
                />
              </div>
            </div>

            {/* 错误 */}
            <Show when={formError()}>
              <div class="text-sm rounded-lg px-3 py-2 bg-red-50 text-red-700 border border-red-200">
                {formError()}
              </div>
            </Show>

            {/* 按钮 */}
            <div class="flex gap-3 pt-2">
              <button
                onClick={closeModal}
                class="flex-1 px-4 py-2 text-sm font-medium text-gray-600 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleSave}
                disabled={saving()}
                class="flex-1 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {saving() ? '保存中...' : '保存'}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}

export default Users
