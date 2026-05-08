/**
 * 判断是否为外部链接
 * @param {string} path - 需要判断的路径
 * @returns {boolean} - 如果是外部链接返回true，否则返回false
 */
export function isExternal(path: string): boolean {
  return /^(https?:|mailto:|tel:)/.test(path);
}

/**
 * 验证邮箱格式
 * @param {string} email - 需要验证的邮箱
 * @returns {boolean} - 如果邮箱格式正确返回true，否则返回false
 */
export function validateEmail(email: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

/**
 * 验证URL格式
 * @param {string} url - 需要验证的URL
 * @returns {boolean} - 如果URL格式正确返回true，否则返回false
 */
export function validateURL(url: string): boolean {
  const reg = /^(https?|ftp):\/\/([a-zA-Z0-9.-]+(:[a-zA-Z0-9.&%$-]+)*@)*((25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9][0-9]?)(\.(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])){3}|([a-zA-Z0-9-]+\.)*[a-zA-Z0-9-]+\.(com|edu|gov|int|mil|net|org|biz|arpa|info|name|pro|aero|coop|museum|[a-zA-Z]{2}))(:[0-9]+)*(\/($|[a-zA-Z0-9.,?'\\+&%$#=~_-]+))*$/;
  return reg.test(url);
}