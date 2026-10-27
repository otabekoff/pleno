/** "2026-10-24" as "October 24, 2026", the same on the server and in the browser. */
export function formatDate(isoDate: string): string {
  return new Date(`${isoDate}T00:00:00Z`).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    timeZone: 'UTC',
  })
}

/**
 * The day a post's frontmatter `date` falls on, as "2026-10-27". A date may
 * carry a time (`2026-10-27T22:30:00+05:00`) to order posts of the same day;
 * the day is counted in the maintainer's time zone, so the time never moves
 * the date a reader sees.
 */
export function postDay(value: string | Date): string {
  return new Date(value).toLocaleDateString('en-CA', { timeZone: 'Asia/Tashkent' })
}
