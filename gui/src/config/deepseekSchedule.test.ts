import { describe, it, expect } from "vitest";
import {
  getDeepSeekPricingStatus,
  getBeijingDate,
  isWeekendBeijing,
  getChinaCalendarStatus,
  getChinaHolidayOverride,
  BUNDLED_CHINA_HOLIDAYS,
  ChinaHolidayYearData,
} from "./deepseekSchedule";
import { CHINA_HOLIDAYS_2026 } from "./chinaHolidays";

describe("deepseekSchedule weekend & peak-valley pricing", () => {
  it("computes Beijing date correctly across UTC midnight / day rollovers", () => {
    // 2026-08-22 20:00 UTC -> 2026-08-23 04:00 Beijing (Saturday UTC -> Sunday Beijing)
    const d1 = new Date("2026-08-22T20:00:00Z");
    const bj1 = getBeijingDate(d1);
    expect(bj1.getUTCFullYear()).toBe(2026);
    expect(bj1.getUTCMonth()).toBe(7); // August (0-indexed)
    expect(bj1.getUTCDate()).toBe(23);
    expect(bj1.getUTCHours()).toBe(4);
    expect(isWeekendBeijing(d1)).toBe(true);

    // 2026-08-21 15:59 UTC -> 2026-08-21 23:59 Beijing (Friday)
    const d2 = new Date("2026-08-21T15:59:00Z");
    expect(isWeekendBeijing(d2)).toBe(false);

    // 2026-08-21 16:00 UTC -> 2026-08-22 00:00 Beijing (Saturday)
    const d3 = new Date("2026-08-21T16:00:00Z");
    expect(isWeekendBeijing(d3)).toBe(true);

    // 2026-08-23 15:59 UTC -> 2026-08-23 23:59 Beijing (Sunday)
    const d4 = new Date("2026-08-23T15:59:00Z");
    expect(isWeekendBeijing(d4)).toBe(true);

    // 2026-08-23 16:00 UTC -> 2026-08-24 00:00 Beijing (Monday)
    const d5 = new Date("2026-08-23T16:00:00Z");
    expect(isWeekendBeijing(d5)).toBe(false);
  });

  it("handles weekend off-peak rule effective boundary (2026-08-22 16:00:00 UTC)", () => {
    // 1 second before effective date (2026-08-22 15:59:59 UTC = Beijing Sat 23:59:59)
    // Outside peak window (15:59 UTC is not 01:00-04:00 or 06:00-10:00) -> VALLEY by normal weekday schedule
    const beforeEffective = new Date("2026-08-22T15:59:59Z");
    const statusBefore = getDeepSeekPricingStatus(beforeEffective);
    expect(statusBefore.period.type).toBe("VALLEY");

    // Saturday BEFORE effective date during standard peak window (2026-08-22 02:00:00 UTC = Beijing Sat 10:00:00)
    // Because weekend rule is not yet effective, standard weekday/weekend peak applies -> PEAK
    const saturdayBeforeEffectivePeak = new Date("2026-08-22T02:00:00Z");
    const statusSatBefore = getDeepSeekPricingStatus(saturdayBeforeEffectivePeak);
    expect(statusSatBefore.period.type).toBe("PEAK");
    expect(statusSatBefore.period.startMinuteUTC).toBe(60);
    expect(statusSatBefore.period.endMinuteUTC).toBe(240);

    // Exactly at effective date: 2026-08-22 16:00:00 UTC (Beijing Sun 00:00:00)
    const atEffective = new Date("2026-08-22T16:00:00Z");
    const statusAt = getDeepSeekPricingStatus(atEffective);
    expect(statusAt.period.type).toBe("VALLEY");
    expect(statusAt.period.startMinuteUTC).toBe(0);
    expect(statusAt.period.endMinuteUTC).toBe(1440);
  });

  it("overrides weekday peak windows on weekends after effective date (Sunday and subsequent Saturday)", () => {
    // Sunday 2026-08-23 02:00:00 UTC (Beijing Sun 10:00:00).
    // On standard schedule, 01:00–04:00 UTC is PEAK.
    // On weekends after effective date, it must be VALLEY.
    const sundayPeakTime = new Date("2026-08-23T02:00:00Z");
    const statusSunday = getDeepSeekPricingStatus(sundayPeakTime);
    expect(statusSunday.period.type).toBe("VALLEY");
    expect(statusSunday.period.startMinuteUTC).toBe(0);
    expect(statusSunday.period.endMinuteUTC).toBe(1440);

    // Sunday 2026-08-23 07:00:00 UTC (Beijing Sun 15:00:00).
    // On standard schedule, 06:00–10:00 UTC is PEAK.
    // On weekends after effective date, it must be VALLEY.
    const sundaySecondPeakTime = new Date("2026-08-23T07:00:00Z");
    const statusSunday2 = getDeepSeekPricingStatus(sundaySecondPeakTime);
    expect(statusSunday2.period.type).toBe("VALLEY");

    // Saturday 2026-08-29 02:00:00 UTC (Beijing Sat 10:00:00).
    // First Saturday AFTER the weekend rule took effect. Must be full-day VALLEY.
    const saturdayAfterEffectivePeak = new Date("2026-08-29T02:00:00Z");
    const statusSatAfter = getDeepSeekPricingStatus(saturdayAfterEffectivePeak);
    expect(statusSatAfter.period.type).toBe("VALLEY");
    expect(statusSatAfter.period.startMinuteUTC).toBe(0);
    expect(statusSatAfter.period.endMinuteUTC).toBe(1440);
  });

  it("resumes normal peak/valley schedule on Monday (Beijing Time)", () => {
    // Monday 2026-08-24 00:30:00 UTC (Beijing Mon 08:30:00) -> outside peak (VALLEY)
    const mondayOffPeak = new Date("2026-08-24T00:30:00Z");
    const statusMonOff = getDeepSeekPricingStatus(mondayOffPeak);
    expect(statusMonOff.period.type).toBe("VALLEY");

    // Monday 2026-08-24 02:00:00 UTC (Beijing Mon 10:00:00) -> inside peak (PEAK)
    const mondayPeak1 = new Date("2026-08-24T02:00:00Z");
    const statusMonPeak1 = getDeepSeekPricingStatus(mondayPeak1);
    expect(statusMonPeak1.period.type).toBe("PEAK");
    expect(statusMonPeak1.period.startMinuteUTC).toBe(60);
    expect(statusMonPeak1.period.endMinuteUTC).toBe(240);

    // Monday 2026-08-24 07:00:00 UTC (Beijing Mon 15:00:00) -> inside peak (PEAK)
    const mondayPeak2 = new Date("2026-08-24T07:00:00Z");
    const statusMonPeak2 = getDeepSeekPricingStatus(mondayPeak2);
    expect(statusMonPeak2.period.type).toBe("PEAK");
    expect(statusMonPeak2.period.startMinuteUTC).toBe(360);
    expect(statusMonPeak2.period.endMinuteUTC).toBe(600);
  });
});

describe("Chapter 5: Chinese holiday and make-up-workday pricing schedule (§18)", () => {
  it("verifies bundled 2026 calendar provenance against official State Council notice", () => {
    const prov = CHINA_HOLIDAYS_2026.provenance;
    expect(prov.year).toBe(2026);
    expect(prov.sourceName).toBe("国务院办公厅");
    expect(prov.officialNoticeTitle).toContain("国务院办公厅关于2026年部分节假日安排的通知");
    expect(prov.officialNoticeTitle).toContain("国办发明电〔2025〕7号");
    expect(prov.officialNoticeUrl).toBe("https://www.gov.cn/zhengce/content/202511/content_7047090.htm");
    expect(prov.officialNoticeDate).toBe("2025-11-04");
    expect(prov.verifiedAt).toBe("2026-10-18");
    expect(prov.datasetUrl).toBe("https://github.com/NateScarlet/holiday-cn/blob/master/2026.json");

    // 2026 is present, 2027 is absent
    expect(BUNDLED_CHINA_HOLIDAYS[2026]).toBeDefined();
    expect(BUNDLED_CHINA_HOLIDAYS[2027]).toBeUndefined();
  });

  it("verifies official 2026 sample entries (New Year, Spring Festival, Qingming, Labor Day, Dragon Boat, Mid-Autumn, National Day)", () => {
    // New Year: 2026-01-01 to 2026-01-03 off; 2026-01-04 make-up working day
    expect(getChinaHolidayOverride("2026-01-01")).toBe(true);
    expect(getChinaHolidayOverride("2026-01-02")).toBe(true);
    expect(getChinaHolidayOverride("2026-01-03")).toBe(true);
    expect(getChinaHolidayOverride("2026-01-04")).toBe(false);

    // Spring Festival: 2026-02-14 make-up; 2026-02-15 to 2026-02-23 off; 2026-02-28 make-up
    expect(getChinaHolidayOverride("2026-02-14")).toBe(false);
    expect(getChinaHolidayOverride("2026-02-15")).toBe(true);
    expect(getChinaHolidayOverride("2026-02-23")).toBe(true);
    expect(getChinaHolidayOverride("2026-02-28")).toBe(false);

    // September / October Mid-Autumn & National Day:
    // 2026-09-20 make-up (Sunday); 2026-09-25 to 2026-09-27 off; 2026-10-01 to 2026-10-07 off; 2026-10-10 make-up (Saturday)
    expect(getChinaHolidayOverride("2026-09-20")).toBe(false);
    expect(getChinaHolidayOverride("2026-09-25")).toBe(true);
    expect(getChinaHolidayOverride("2026-09-26")).toBe(true);
    expect(getChinaHolidayOverride("2026-09-27")).toBe(true);
    expect(getChinaHolidayOverride("2026-10-01")).toBe(true);
    expect(getChinaHolidayOverride("2026-10-07")).toBe(true);
    expect(getChinaHolidayOverride("2026-10-10")).toBe(false);
  });

  it("classifies verified weekday off-day as full-day VALLEY even during UTC peak hours", () => {
    // 2026-01-01 is Thursday (weekday) and New Year's Day (off-day).
    // 02:00 UTC (10:00 Beijing) is normally in peak window (01:00-04:00 UTC).
    // Because it's an off-day, it must be full-day VALLEY.
    const holidayPeakTime = new Date("2026-01-01T02:00:00Z");
    const status = getDeepSeekPricingStatus(holidayPeakTime);
    expect(status.period.type).toBe("VALLEY");
    expect(status.period.startMinuteUTC).toBe(0);
    expect(status.period.endMinuteUTC).toBe(1440);

    // Same for afternoon peak window (07:00 UTC = 15:00 Beijing)
    const holidayPeakTime2 = new Date("2026-01-01T07:00:00Z");
    const status2 = getDeepSeekPricingStatus(holidayPeakTime2);
    expect(status2.period.type).toBe("VALLEY");
    expect(status2.period.startMinuteUTC).toBe(0);
    expect(status2.period.endMinuteUTC).toBe(1440);

    // Outside peak window on holiday weekday: also VALLEY
    const holidayOffPeakTime = new Date("2026-01-01T05:00:00Z");
    const statusOff = getDeepSeekPricingStatus(holidayOffPeakTime);
    expect(statusOff.period.type).toBe("VALLEY");
    expect(statusOff.period.startMinuteUTC).toBe(0);
    expect(statusOff.period.endMinuteUTC).toBe(1440);
  });

  it("applies ordinary peak ranges on make-up working days falling on weekend (Sunday and Saturday)", () => {
    // 2026-01-04 is Sunday (make-up working day, isOffDay: false).
    // Inside peak window 1 (02:00 UTC = 10:00 Beijing) -> must be PEAK
    const makeupPeak1 = new Date("2026-01-04T02:00:00Z");
    const statusPeak1 = getDeepSeekPricingStatus(makeupPeak1);
    expect(statusPeak1.period.type).toBe("PEAK");
    expect(statusPeak1.period.startMinuteUTC).toBe(60);
    expect(statusPeak1.period.endMinuteUTC).toBe(240);

    // Inside peak window 2 (07:00 UTC = 15:00 Beijing) -> must be PEAK
    const makeupPeak2 = new Date("2026-01-04T07:00:00Z");
    const statusPeak2 = getDeepSeekPricingStatus(makeupPeak2);
    expect(statusPeak2.period.type).toBe("PEAK");
    expect(statusPeak2.period.startMinuteUTC).toBe(360);
    expect(statusPeak2.period.endMinuteUTC).toBe(600);

    // Outside peak window on make-up Sunday (00:30 UTC = 08:30 Beijing) -> VALLEY (not full-day)
    const makeupValley = new Date("2026-01-04T00:30:00Z");
    const statusValley = getDeepSeekPricingStatus(makeupValley);
    expect(statusValley.period.type).toBe("VALLEY");
    expect(statusValley.period.startMinuteUTC).toBe(600);
    expect(statusValley.period.endMinuteUTC).toBe(60);

    // 2026-09-20 is Sunday make-up working day
    const sep20Peak = new Date("2026-09-20T02:00:00Z");
    expect(getDeepSeekPricingStatus(sep20Peak).period.type).toBe("PEAK");
    const sep20Valley = new Date("2026-09-20T05:00:00Z");
    expect(getDeepSeekPricingStatus(sep20Valley).period.type).toBe("VALLEY");

    // 2026-10-10 is Saturday make-up working day
    const oct10Peak = new Date("2026-10-10T02:00:00Z");
    expect(getDeepSeekPricingStatus(oct10Peak).period.type).toBe("PEAK");
    const oct10Valley = new Date("2026-10-10T05:00:00Z");
    expect(getDeepSeekPricingStatus(oct10Valley).period.type).toBe("VALLEY");
  });

  it("preserves standard peak/valley rules for ordinary non-holiday weekday and ordinary weekend", () => {
    // Ordinary Wednesday 2026-08-26: not in holiday calendar
    expect(getChinaHolidayOverride("2026-08-26")).toBeUndefined();
    // Peak at 02:00 UTC
    const wedPeak = new Date("2026-08-26T02:00:00Z");
    expect(getDeepSeekPricingStatus(wedPeak).period.type).toBe("PEAK");
    // Valley at 05:00 UTC
    const wedValley = new Date("2026-08-26T05:00:00Z");
    expect(getDeepSeekPricingStatus(wedValley).period.type).toBe("VALLEY");

    // Ordinary Sunday 2026-08-30: not in holiday calendar, after effective date
    expect(getChinaHolidayOverride("2026-08-30")).toBeUndefined();
    const sunInstant = new Date("2026-08-30T02:00:00Z");
    const sunStatus = getDeepSeekPricingStatus(sunInstant);
    expect(sunStatus.period.type).toBe("VALLEY");
    expect(sunStatus.period.startMinuteUTC).toBe(0);
    expect(sunStatus.period.endMinuteUTC).toBe(1440);
  });

  it("retains documented fallback for dates without bundled year/data entry (e.g. 2027)", () => {
    // 2027 has no bundled data. Must NOT silently treat dates as holidays.
    // 2027-01-04 is Monday: peak at 02:00 UTC must be PEAK
    const mon2027 = new Date("2027-01-04T02:00:00Z");
    expect(getDeepSeekPricingStatus(mon2027).period.type).toBe("PEAK");

    // 2027-01-03 is Sunday: weekend all-day valley
    const sun2027 = new Date("2027-01-03T02:00:00Z");
    const sun2027Status = getDeepSeekPricingStatus(sun2027);
    expect(sun2027Status.period.type).toBe("VALLEY");
    expect(sun2027Status.period.startMinuteUTC).toBe(0);
    expect(sun2027Status.period.endMinuteUTC).toBe(1440);
  });

  it("maps UTC instants near midnight and year boundaries to correct Shanghai-local date", () => {
    // 2025-12-31 15:59:59 UTC = 2025-12-31 23:59:59 Beijing (Wednesday, 2025, no 2026 holiday yet)
    const tBeforeRoll = new Date("2025-12-31T15:59:59Z");
    const bjBefore = getBeijingDate(tBeforeRoll);
    expect(bjBefore.getUTCFullYear()).toBe(2025);
    expect(bjBefore.getUTCMonth()).toBe(11);
    expect(bjBefore.getUTCDate()).toBe(31);

    // 2025-12-31 16:00:00 UTC = 2026-01-01 00:00:00 Beijing (New Year's Day, off-day)
    const tNewYear = new Date("2025-12-31T16:00:00Z");
    const bjNewYear = getBeijingDate(tNewYear);
    expect(bjNewYear.getUTCFullYear()).toBe(2026);
    expect(bjNewYear.getUTCMonth()).toBe(0);
    expect(bjNewYear.getUTCDate()).toBe(1);
    expect(getDeepSeekPricingStatus(tNewYear).period.type).toBe("VALLEY");

    // 2026-01-03 16:00:00 UTC = 2026-01-04 00:00:00 Beijing (Sunday make-up workday)
    const tMakeupStart = new Date("2026-01-03T16:00:00Z");
    const bjMakeup = getBeijingDate(tMakeupStart);
    expect(bjMakeup.getUTCFullYear()).toBe(2026);
    expect(bjMakeup.getUTCDate()).toBe(4);
    // At 01:30 UTC on 2026-01-04 (09:30 Beijing), it's inside peak
    const tMakeupPeak = new Date("2026-01-04T01:30:00Z");
    expect(getDeepSeekPricingStatus(tMakeupPeak).period.type).toBe("PEAK");
  });

  it("determines calendar maintenance status transitions (Nov 30, Dec 1, mock next year, Jan 1 expired)", () => {
    const calendar2026Only: Record<number, ChinaHolidayYearData> = {
      2026: CHINA_HOLIDAYS_2026,
    };

    // 1. 2026-11-30 with 2026 data only -> current
    const nov30 = new Date(2026, 10, 30, 12, 0, 0); // month is 0-indexed: 10 = Nov
    expect(getChinaCalendarStatus(nov30, calendar2026Only)).toEqual({
      status: "current",
    });

    // 2. 2026-12-01 with 2026 data only -> update_due, targetYear 2027
    const dec01 = new Date(2026, 11, 1, 0, 0, 0); // 11 = Dec
    expect(getChinaCalendarStatus(dec01, calendar2026Only)).toEqual({
      status: "update_due",
      targetYear: 2027,
    });

    // 3. 2026-12-01 with mock 2027 data present -> current
    const calendarWith2027: Record<number, ChinaHolidayYearData> = {
      ...calendar2026Only,
      2027: {
        provenance: {
          year: 2027,
          sourceName: "国务院办公厅",
          officialNoticeTitle: "Mock 2027",
          officialNoticeUrl: "https://example.com",
          officialNoticeDate: "2026-11-15",
          verifiedAt: "2026-11-16",
          datasetUrl: "https://example.com/2027.json",
        },
        days: {},
      },
    };
    expect(getChinaCalendarStatus(dec01, calendarWith2027)).toEqual({
      status: "current",
    });

    // 4. 2027-01-01 when 2027 data remains absent -> expired, targetYear 2027
    const jan01_2027 = new Date(2027, 0, 1, 0, 0, 0);
    expect(getChinaCalendarStatus(jan01_2027, calendar2026Only)).toEqual({
      status: "expired",
      targetYear: 2027,
    });
  });
});
