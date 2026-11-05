# Incorrect instrument audio beneath expected MP3 filename

## Observed failure

During local HTTP downloading of purchased Karaoke-Version custom backing-track stems, responses sometimes contained audio for a previously requested instrument despite a plausible filename for the current instrument. This is a **client-visible stale-response problem**, but it does **not** conclusively identify its root cause as the remote service rather than client, session, or request sequencing. Examples included Click/Drum Kit/Percussion swaps during an 11-track run of **America — A Horse with No Name**. This failure could persist across separate download sessions.

Simple filename checks, equal file lengths, and nonmatching MP3 hashes were insufficient. Exact duplicate detection caught some stale responses, but a re-encoded stale mix could evade bytewise comparison.

## Diagnostic evidence

- In affected responses, the purported new instrument sometimes had the exact ETag and Last-Modified value of the immediately preceding accepted instrument.
- Earlier stale content could also appear on the first track of a later session, when no current-run duplicate existed.
- Correct retries generally received later modification timestamps and different ETags.
- The browser method succeeded in an 11-track listening check; its download workflow watched the website for the offered filename to change to the requested instrument.

## C22 mitigation

HTTP polling now checks requested filename and key, MP3 responses undergo server-time freshness checks, reused ETags are rejected, and bytewise duplicate detection remains in place. Failing requests use existing bounded retry behavior. Unexpected MP3 HTTP status codes are refused.

## Results and boundaries

The C22C-2 HTTP live listening runs returned correct instrument audio for all requested tracks in the reported tests, with multiple stale resources rejected. Those live HTTP runs **preceded** addition of the final explicit HTTP 200 response-status check; that last guard was included in passing automated tests and Release build, but no subsequent live HTTP run using the archived GUI executable was performed. These safeguards **reduce risk**; they do not verify the audio signal itself or prove that all future website responses will be musically correct.
