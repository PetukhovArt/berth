const DATE_BUCKETS = ['today', 'yesterday', 'week', 'month', 'older'] as const;
export type DateBucket = (typeof DATE_BUCKETS)[number];

const FLAT_LIMIT = 5;

type DateGroup<T> = { bucket: DateBucket; items: T[]; open: boolean };
export type DateGrouping<T> = { loose: T[]; flat: { item: T; bucket: DateBucket }[]; groups: DateGroup<T>[] };

const dayNumber = (date: Date) => Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000);

function bucketOf(time: number, now: Date): DateBucket {
  const days = dayNumber(now) - dayNumber(new Date(time));
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days <= 7) return 'week';
  if (days <= 30) return 'month';
  return 'older';
}

export function groupByDate<T>(items: readonly T[], timeOf: (item: T) => number | null, now: Date): DateGrouping<T> {
  const loose: T[] = [];
  const dated: { item: T; time: number }[] = [];
  for (const item of items) {
    const time = timeOf(item);
    if (time === null || !Number.isFinite(time) || time <= 0) loose.push(item);
    else dated.push({ item, time });
  }
  dated.sort((a, b) => b.time - a.time);
  if (dated.length <= FLAT_LIMIT) return { loose, flat: dated.map(({ item, time }) => ({ item, bucket: bucketOf(time, now) })), groups: [] };
  const byBucket = new Map<DateBucket, T[]>();
  for (const { item, time } of dated) {
    const bucket = bucketOf(time, now);
    byBucket.set(bucket, [...(byBucket.get(bucket) ?? []), item]);
  }
  const present = DATE_BUCKETS.filter((bucket) => byBucket.has(bucket));
  const groups = present.map((bucket) => ({ bucket, items: byBucket.get(bucket) ?? [], open: bucket === 'today' || bucket === 'yesterday' || bucket === present[0] }));
  return { loose, flat: [], groups };
}
