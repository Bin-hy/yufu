import { ref } from "vue";
import { userApi, type UserVO } from "@/api/users";

const nicknameMap = ref<Record<number, string>>({});
const users = ref<UserVO[]>([]);
let loaded = false;

export function useNicknames() {
  async function loadAll() {
    if (loaded) return;
    loaded = true;
    try {
      const res = await userApi.fetchAll();
      const data = res.data.data;
      if (data) {
        const map: Record<number, string> = {};
        for (const u of data.users) {
          map[u.id] = u.nickname || u.username;
        }
        nicknameMap.value = map;
        users.value = data.users;
      }
    } catch {
      loaded = false;
    }
  }

  function get(id: number): string {
    return nicknameMap.value[id] || `用户 #${id}`;
  }

  return { nicknameMap, users, loadAll, get };
}
