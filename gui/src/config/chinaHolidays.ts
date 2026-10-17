// Static calendar data and status helper for Chinese public holidays and make-up workdays.
// Authoritative basis: State Council of the People's Republic of China annual holiday notices.
// Bounded static calendar per plan §18. Only officially published and verified dates are included.

export interface ChinaHolidayYearProvenance {
  year: number;
  sourceName: string;
  officialNoticeTitle: string;
  officialNoticeUrl: string;
  officialNoticeDate: string;
  verifiedAt: string;
  datasetUrl: string;
}

export interface ChinaHolidayYearData {
  provenance: ChinaHolidayYearProvenance;
  /** ISO date "YYYY-MM-DD" -> boolean (true = statutory/adjusted off-day, false = make-up working day) */
  days: Record<string, boolean>;
}

export type ChinaCalendarStatus = "current" | "update_due" | "expired";

export interface ChinaCalendarStatusResult {
  status: ChinaCalendarStatus;
  targetYear?: number;
}

export const CHINA_HOLIDAYS_2026: ChinaHolidayYearData = {
  provenance: {
    year: 2026,
    sourceName: "国务院办公厅",
    officialNoticeTitle: "国务院办公厅关于2026年部分节假日安排的通知 (国办发明电〔2025〕7号)",
    officialNoticeUrl: "https://www.gov.cn/zhengce/content/202511/content_7047090.htm",
    officialNoticeDate: "2025-11-04",
    verifiedAt: "2026-10-18",
    datasetUrl: "https://github.com/NateScarlet/holiday-cn/blob/master/2026.json",
  },
  days: {
    // New Year (元旦): 2026-01-01 to 2026-01-03 off; 2026-01-04 make-up working day
    "2026-01-01": true,
    "2026-01-02": true,
    "2026-01-03": true,
    "2026-01-04": false,

    // Spring Festival (春节): 2026-02-14 make-up; 2026-02-15 to 2026-02-23 off; 2026-02-28 make-up
    "2026-02-14": false,
    "2026-02-15": true,
    "2026-02-16": true,
    "2026-02-17": true,
    "2026-02-18": true,
    "2026-02-19": true,
    "2026-02-20": true,
    "2026-02-21": true,
    "2026-02-22": true,
    "2026-02-23": true,
    "2026-02-28": false,

    // Qingming (清明节): 2026-04-04 to 2026-04-06 off
    "2026-04-04": true,
    "2026-04-05": true,
    "2026-04-06": true,

    // Labor Day (劳动节): 2026-05-01 to 2026-05-05 off; 2026-05-09 make-up
    "2026-05-01": true,
    "2026-05-02": true,
    "2026-05-03": true,
    "2026-05-04": true,
    "2026-05-05": true,
    "2026-05-09": false,

    // Dragon Boat (端午节): 2026-06-19 to 2026-06-21 off
    "2026-06-19": true,
    "2026-06-20": true,
    "2026-06-21": true,

    // Mid-Autumn (中秋节): 2026-09-20 make-up; 2026-09-25 to 2026-09-27 off
    "2026-09-20": false,
    "2026-09-25": true,
    "2026-09-26": true,
    "2026-09-27": true,

    // National Day (国庆节): 2026-10-01 to 2026-10-07 off; 2026-10-10 make-up
    "2026-10-01": true,
    "2026-10-02": true,
    "2026-10-03": true,
    "2026-10-04": true,
    "2026-10-05": true,
    "2026-10-06": true,
    "2026-10-07": true,
    "2026-10-10": false,
  },
};

export const BUNDLED_CHINA_HOLIDAYS: Record<number, ChinaHolidayYearData> = {
  2026: CHINA_HOLIDAYS_2026,
};

/**
 * Returns holiday override for a "YYYY-MM-DD" date string in Beijing Time:
 * true = statutory/adjusted off-day (all-day VALLEY)
 * false = make-up workday (treated as working day even if Saturday/Sunday)
 * undefined = not explicitly recorded in bundled calendar (fallback to standard weekday/weekend rule)
 */
export function getChinaHolidayOverride(
  dateStr: string,
  calendars: Record<number, ChinaHolidayYearData> = BUNDLED_CHINA_HOLIDAYS,
): boolean | undefined {
  const year = parseInt(dateStr.slice(0, 4), 10);
  const calendar = calendars[year];
  if (!calendar) return undefined;
  return calendar.days[dateStr];
}

/**
 * Pure helper determining China holiday calendar maintenance status from system-local date.
 * Deterministic with injected `now` Date and `calendars` map.
 */
export function getChinaCalendarStatus(
  now: Date,
  calendars: Record<number, ChinaHolidayYearData> = BUNDLED_CHINA_HOLIDAYS,
): ChinaCalendarStatusResult {
  const currentYear = now.getFullYear();
  const nextYear = currentYear + 1;

  if (!calendars[currentYear]) {
    return {
      status: "expired",
      targetYear: currentYear,
    };
  }

  // Current year exists. Check if today is December 1 or later.
  // Note: getMonth() is 0-indexed (November = 10, December = 11).
  const isDecOrLater = now.getMonth() === 11 && now.getDate() >= 1;
  if (isDecOrLater && !calendars[nextYear]) {
    return {
      status: "update_due",
      targetYear: nextYear,
    };
  }

  return {
    status: "current",
  };
}
