import { getUserInfo, login as loginService } from '@/service';

export const getUser = async () => {
  try {
    const result = await getUserInfo();
    if (result.success && result.data) {
      return result.data;
    }
    return null;
  } catch {
    return null;
  }
};

export const login = async (
  username: string,
  password: string,
): Promise<{
  success: boolean;
}> => {
  try {
    const result = await loginService(username, password);
    if (result.success) {
      //window.open('/loading', '_self');
      return result;
    }
    throw new Error(result.message || 'Login failed');
  } catch (e) {
    throw new Error((e as Error).message);
  }
};
