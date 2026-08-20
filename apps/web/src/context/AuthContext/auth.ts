import { getUserInfo } from '@/service';

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
