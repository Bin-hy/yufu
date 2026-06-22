/**
 * 消息时间格式化工具
 *
 * 根据消息时间与当前时间的距离，返回不同精度的显示文本：
 * - 今天 → HH:mm
 * - 昨天 → 昨天 HH:mm
 * - 本周内 → 周X HH:mm
 * - 本年内 → MM-DD HH:mm
 * - 更早   → YYYY-MM-DD HH:mm
 */

const WEEKDAY_NAMES = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

/**
 * 获取某天零点的 Date 对象（本地时间）
 */
function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

/**
 * 格式化消息时间。
 * @param isoStr ISO 8601 时间字符串
 * @returns 格式化后的时间文本
 */
export function formatMessageTime(isoStr: string): string {
  const d = new Date(isoStr);
  if (isNaN(d.getTime())) return "";

  const now = new Date();
  const msgStart = startOfDay(d);
  const todayStart = startOfDay(now);
  const diffDays = Math.round((todayStart.getTime() - msgStart.getTime()) / 86_400_000);

  const timePart = d.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });

  if (diffDays === 0) {
    // 今天
    return timePart;
  } else if (diffDays === 1) {
    // 昨天
    return `昨天 ${timePart}`;
  } else if (diffDays < 7 && d.getDay() !== now.getDay()) {
    // 本周内（非今天/昨天）
    return `${WEEKDAY_NAMES[d.getDay()]} ${timePart}`;
  } else if (d.getFullYear() === now.getFullYear()) {
    // 本年内
    const month = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${month}-${day} ${timePart}`;
  } else {
    // 跨年
    const year = d.getFullYear();
    const month = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${year}-${month}-${day} ${timePart}`;
  }
}

/**
 * 检查两条消息是否在同一天。
 * 用于 ChatWindow 中判断是否需要在消息间插入日期分隔符。
 */
export function isSameDay(a: string, b: string): boolean {
  const da = new Date(a);
  const db = new Date(b);
  if (isNaN(da.getTime()) || isNaN(db.getTime())) return true;
  return (
    da.getFullYear() === db.getFullYear() &&
    da.getMonth() === db.getMonth() &&
    da.getDate() === db.getDate()
  );
}

/**
 * 获取日期分隔符文本（用于消息列表中的日期标记）。
 */
export function formatDateSeparator(isoStr: string): string {
  const d = new Date(isoStr);
  if (isNaN(d.getTime())) return "";

  const now = new Date();
  const msgStart = startOfDay(d);
  const todayStart = startOfDay(now);
  const diffDays = Math.round((todayStart.getTime() - msgStart.getTime()) / 86_400_000);

  if (diffDays === 0) return "今天";
  if (diffDays === 1) return "昨天";
  if (diffDays < 7 && d.getDay() !== now.getDay()) return WEEKDAY_NAMES[d.getDay()];

  const year = d.getFullYear();
  const month = d.getMonth() + 1;
  const day = d.getDate();
  if (year === now.getFullYear()) {
    return `${month}月${day}日`;
  }
  return `${year}年${month}月${day}日`;
}
