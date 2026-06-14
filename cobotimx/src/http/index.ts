import axios from "axios";

const http = axios.create({
  baseURL: "/api/v1",
  timeout: 30_000,
  headers: { "Content-Type": "application/json" },
});

http.interceptors.request.use((config) => {
  const token = sessionStorage.getItem("access_token");
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

http.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error.config;
    if (
      error.response?.status === 401 &&
      !original._retry &&
      original.url !== "/refresh"
    ) {
      original._retry = true;
      try {
        const { data } = await axios.post("/api/v1/refresh", null, {
          withCredentials: true,
        });
        const token = data.access_token;
        if (token) {
          sessionStorage.setItem("access_token", token);
          original.headers.Authorization = `Bearer ${token}`;
          return http(original);
        }
      } catch {
        sessionStorage.removeItem("access_token");
        window.location.href = "/login";
      }
    }
    return Promise.reject(error);
  },
);

export default http;
