# DOMAINS.md

**Project:** IDS — Independent Distributed System  
**Parent ecosystem:** HX-241 / XarnX  
**Scope:** 600 failure domains across 20 layers where IDS could go wrong  
**Status:** Conceptual risk inventory; not implementation results  
**Purpose:** Enumerate categories of failure, not individual cases. Each domain is a boundary where the architecture must demonstrate handling.

---

## Layer 1: Network Transport & Link (1–30)

1. Partial network partition — some nodes can reach each other, others cannot  
2. Asymmetric partition — A reaches B, B does not reach A  
3. Full partition between two regions  
4. Top-of-rack switch failure isolating an H-unit  
5. Core router failure isolating a region  
6. Misconfigured firewall blocking cross-region traffic  
7. One-way black hole — A→B works, B→A fails  
8. Intermittent packet loss on a single link  
9. Burst packet loss during congestion  
10. MTU mismatch causing silent packet drops  
11. Route flapping between H-units  
12. Unidirectional link failure  
13. Duplex mismatch causing collisions and late drops  
14. Bufferbloat on cross-region links  
15. NIC driver crash leaving link up but dead  
16. Transceiver failure on one port of a redundant pair  
17. Cable cut between buildings in the same region  
18. Fiber cut between regions  
19. Satellite link degradation due to weather  
20. Radio interference on wireless carrier  
21. Power over Ethernet failure to edge node  
22. VLAN misconfiguration isolating an H-unit  
23. SDN controller crash breaking forwarding  
24. Load balancer sending traffic to a dead node  
25. NAT table exhaustion on gateway  
26. Connection tracking table full dropping new sessions  
27. Jitter causing spurious failure detection  
28. Congestion collapse on shared uplink  
29. NIC ring buffer overflow dropping packets  
30. DMA error corrupting in-flight packets  

---

## Layer 2: Addressing & Naming (31–60)

31. DNS resolution failure for HX-241 native names  
32. DNS cache poisoning pointing to malicious node  
33. DNS TTL too long causing stale address use  
34. Native address collision between two nodes  
35. Address reuse after node retirement before cache expiry  
36. Address spoofing by malicious node  
37. Address resolution service crash  
38. Address resolution service returning stale entries  
39. Location-independent address not resolving after node move  
40. Address mapping partition — half the network sees old mapping  
41. Name-to-address resolution loop  
42. Hierarchical name delegation failure between regions  
43. Name collision across independent regional clusters  
44. Address namespace exhaustion  
45. Address not updated after H-unit reassignment  
46. Client caching stale address after failover  
47. Anycast prefix withdrawal making region unreachable  
48. Protocol ossification blocking new HX-241 frame types  
49. Middlebox altering or dropping HX-241 frames  
50. Captive portal intercepting bootstrap traffic  
51. TLS termination point doing unexpected routing  
52. Certificate expiration breaking mutual auth  
53. Certificate revocation list unreachable  
54. OCSP stapling failure causing handshake hang  
55. Cipher suite mismatch between nodes  
56. TLS session ticket key rotation breaking resumption  
57. Perfect forward secrecy key loss  
58. Replay of old handshake packets  
59. Fragmentation of encrypted packets causing reassembly failure  
60. Packet duplication on redundant network paths  

---

## Layer 3: Carrier & Physical Medium (61–90)

61. Carrier medium physical layer failure  
62. Experimental carrier link flapping  
63. Carrier medium electromagnetic interference  
64. Carrier medium physical tampering  
65. Carrier medium signal attenuation beyond threshold  
66. Carrier medium introducing bit errors undetected by link layer  
67. Carrier medium asymmetric bandwidth — upload works, download dead  
68. Carrier medium introducing delay variation (jitter)  
69. Carrier medium shared medium contention  
70. Carrier medium node physically relocated without notification  
71. Carrier medium power failure at edge endpoint  
72. Carrier medium weather-related outage  
73. Carrier medium construction damage  
74. Carrier medium animal damage to physical line  
75. Carrier medium connector corrosion causing intermittent loss  
76. Carrier medium device firmware incompatibility  
77. Carrier medium protocol version mismatch at physical layer  
78. Carrier medium handshake timeout under load  
79. Carrier medium frame corruption undetected  
80. Carrier medium link state flapping rapidly  
81. Carrier medium quality of service misconfiguration  
82. Carrier medium priority inversion — low priority traffic blocking high  
83. Carrier medium admission control rejecting legitimate frames  
84. Carrier medium resource exhaustion at physical layer  
85. Carrier medium eavesdropping on unencrypted frames  
86. Carrier medium frame injection by attacker  
87. Carrier medium replay of captured frames  
88. Carrier medium jamming attack  
89. Carrier medium spoofing of endpoint identity  
90. Carrier medium disconnect during critical operation  

---

## Layer 4: Node Hardware & Firmware (91–120)

91. Node process crash  
92. Node kernel panic  
93. Node OS hang  
94. Node power loss  
95. Power supply failure on redundant PSU  
96. UPS battery depletion during outage  
97. PDU circuit breaker trip  
98. Datacenter power grid failure  
99. Generator failure to start  
100. CPU overheating causing thermal throttle  
101. CPU socket failure killing all cores  
102. Memory module failure  
103. Memory bit flip uncorrected  
104. Memory corruption from row hammer  
105. CPU cache corruption  
106. Microcode bug causing incorrect computation  
107. Instruction set extension unavailable on older node  
108. NIC firmware bug dropping packets under load  
109. NIC hardware queue exhaustion  
110. NIC transceiver failure  
111. Disk drive failure  
112. Disk latent sector error  
113. Disk media error returning garbage  
114. Disk firmware hang  
115. Disk write cache lying about persistence  
116. RAID controller failure  
117. SSD wear out reaching endurance limit  
118. SSD power loss capacitor failure losing data  
119. NVMe controller overheating throttling  
120. BIOS/UEFI firmware bug  

---

## Layer 5: Node OS & Runtime (121–150)

121. Firmware update bricking node  
122. SMM long pause freezing OS  
123. IPMI/BMC unresponsive  
124. BMC firmware compromised  
125. Serial console stuck  
126. Rack PDU overload tripping  
127. Rack cooling failure causing thermal shutdown  
128. Water leak shorting equipment  
129. Fire suppression system shutting down power  
130. Seismic event moving racks  
131. Physical theft of node  
132. Unauthorized physical access to console  
133. Supply chain compromise of hardware  
134. Counterfeit components in node  
135. Virtual machine host crash killing all guests  
136. Hypervisor bug corrupting guest memory  
137. VM escape attack  
138. Container runtime crash  
139. Container escape  
140. Orchestrator control plane down  
141. Scheduler placing conflicting pods on same node  
142. Node taint preventing pod scheduling  
143. Pod eviction during node pressure  
144. OOM killer terminating database process  
145. cgroup CPU throttling slowing node below SLA  
146. cgroup memory limit causing OOM  
147. PID exhaustion preventing new processes  
148. File descriptor exhaustion preventing new connections  
149. Thread exhaustion  
150. Process zombie pileup  

---

## Layer 6: Node Memory & CPU (151–180)

151. Daemon crash loop  
152. Init system failure  
153. Syslog exhaustion losing logs  
154. Auditd failure losing security records  
155. Kernel module load failure  
156. Driver mismatch after kernel upgrade  
157. SELinux/AppArmor blocking legitimate operation  
158. Firewall rule conflict  
159. Secure Boot blocking unsigned kernel module  
160. TPM malfunction preventing sealed key unsealing  
161. Hardware RNG failure producing predictable keys  
162. RDRAND/RDSEED instruction unavailable or biased  
163. CPU frequency scaling causing variable latency  
164. CPU pinning to isolated cores broken  
165. NUMA spanning causing remote memory access  
166. Huge pages allocation failure  
167. Transparent huge page causing memory spikes  
168. Kernel samepage merging causing side channel  
169. Memory leak slowly consuming all RAM  
170. Swap thrashing under memory pressure  
171. NUMA node imbalance causing remote memory latency  
172. Cache coherence protocol bug  
173. CPU cache poisoned by malicious code  
174. Page table corruption  
175. TLB shootdown failure  
176. Huge page split causing fragmentation  
177. Swap partition corruption  
178. Swap file on failing disk  
179. Memory-mapped file corruption  
180. Buffer overflow in storage daemon  

---

## Layer 7: Storage Media & Filesystems (181–210)

181. Use-after-free in storage daemon  
182. Null pointer dereference in storage path  
183. Integer overflow in block allocation  
184. Off-by-one in write truncating last byte  
185. Race condition in file create/delete  
186. TOCTOU in permission check  
187. Symbolic link escaping containment  
188. Hard link to sensitive file created by user  
189. Bind mount exposing host paths  
190. Chroot escape  
191. Overlayfs whiteout corruption  
192. Overlayfs upper layer full  
193. Container image layer corrupted  
194. Container image tampered  
195. Container image registry unavailable  
196. Image pull rate limit exceeded  
197. Volume mount failure  
198. Volume already mounted elsewhere  
199. Volume permission denied  
200. Volume ownership mismatch  
201. SELinux context wrong on volume  
202. AppArmor profile blocking volume access  
203. Storage class unavailable  
204. Storage provisioner crash  
205. CSI driver failure  
206. CSI controller pod crash looping  
207. CSI node plugin not registered  
208. Silent bit rot on disk  
209. Checksum mismatch on read  
210. Misdirected write to wrong block  

---

## Layer 8: Storage Metadata & Indexing (211–240)

211. Lost write acknowledged but not persisted  
212. Filesystem metadata corruption  
213. Journal corruption preventing filesystem mount  
214. Superblock corruption  
215. Inode corruption  
216. Directory entry corruption  
217. Block bitmap corruption  
218. Extent map corruption  
219. Filesystem full (ENOSPC)  
220. Quota exceeded preventing writes  
221. Read-only filesystem after remount  
222. Filesystem feature incompatibility after upgrade  
223. Sparse file hole read returning zeros incorrectly  
224. Deduplication hash collision losing data  
225. Compression algorithm bug corrupting data  
226. Encryption key rotation failing mid-write  
227. Encrypted data unreadable after key loss  
228. Key management service unreachable  
229. Hardware security module failure  
230. Key wrapping key lost  
231. Key backup unrecoverable  
232. Key rotation interval too short causing outage  
233. Key rotation interval too long increasing exposure  
234. Key used for wrong purpose  
235. Silent data corruption in memory before write  
236. DMA to wrong memory address  
237. ECC memory correctable error storm  
238. ECC memory uncorrectable error crashing node  
239. Memory scrubber missing error window  
240. Cache coherence protocol bug  

---

## Layer 9: Data Integrity & Corruption (241–270)

241. CPU cache poisoned by malicious code  
242. Page table corruption  
243. TLB shootdown failure  
244. Huge page split causing fragmentation  
245. Swap partition corruption  
246. Swap file on failing disk  
247. Memory-mapped file corruption  
248. Buffer overflow in storage daemon  
249. Use-after-free in storage daemon  
250. Null pointer dereference in storage path  
251. Integer overflow in block allocation  
252. Off-by-one in write truncating last byte  
253. Race condition in file create/delete  
254. TOCTOU in permission check  
255. Symbolic link escaping containment  
256. Hard link to sensitive file created by user  
257. Bind mount exposing host paths  
258. Chroot escape  
259. Overlayfs whiteout corruption  
260. Overlayfs upper layer full  
261. Container image layer corrupted  
262. Container image tampered  
263. Container image registry unavailable  
264. Image pull rate limit exceeded  
265. Volume mount failure  
266. Volume already mounted elsewhere  
267. Volume permission denied  
268. Volume ownership mismatch  
269. SELinux context wrong on volume  
270. AppArmor profile blocking volume access  

---

## Layer 10: Backup & Restore (271–300)

271. Storage class unavailable  
272. Storage provisioner crash  
273. CSI driver failure  
274. CSI controller pod crash looping  
275. CSI node plugin not registered  
276. CSI volume snapshot failure  
277. CSI volume clone failure  
278. CSI volume expansion failure  
279. CSI volume resize online not supported  
280. Storage pool degraded  
281. Storage pool offline  
282. Storage pool rebalancing stuck  
283. Storage pool import failure  
284. Storage pool export failure  
285. ZFS resilver slow under load  
286. ZFS scrub causing performance degradation  
287. ZFS dedup table too large for RAM  
288. Btrfs balance interrupted  
289. Btrfs scrub finding unrecoverable errors  
290. ext4 delayed allocation data loss on crash  
291. XFS log corruption  
292. XFS allocation group corruption  
293. NTFS journal corruption  
294. FAT file system cluster chain corruption  
295. exFAT allocation bitmap corruption  
296. Optical disc read error  
297. Tape drive read error during restore  
298. Tape media degradation  
299. Offline backup missing  
300. Restore test never performed  

---

## Layer 11: Replication & Synchronization (301–330)

301. Offline backup corrupted  
302. Offline backup encrypted by ransomware  
303. Backup retention policy deleting needed version  
304. Backup verification never run  
305. Replication lag exceeding threshold  
306. Replication stream interrupted mid-transfer  
307. Partial replica state after interrupted transfer  
308. Replica divergence from primary  
309. Split-brain: two primaries accepting writes  
310. Leader election failure  
311. Leader stepped down during active write  
312. Leader partitioned from majority but still accepting writes  
313. Quorum not reachable  
314. Quorum split across failed network  
315. Two-phase commit blocking on participant failure  
316. Three-phase commit blocking on coordinator failure  
317. Paxos proposer starvation  
318. Raft log compaction removing needed entries  
319. Raft snapshot installation failure  
320. Raft log mismatch between leader and follower  
321. Raft snapshot mismatch causing follower to reject  
322. Zab leader not synchronized with followers  
323. Zab epoch mismatch  
324. View change in PBFT taking too long  
325. PBFT primary faulty and not detected  
326. HotStuff view change deadlock  
327. Chain replication chain broken  
328. Chain replication tail not caught up  
329. Primary-backup failover too slow  
330. Backup promoted but missing last writes  

---

## Layer 12: Consistency & Ordering (331–360)

331. Active-passive split-brain on network glitch  
332. Multi-master conflict not detected  
333. Multi-master conflict resolution policy missing  
334. Last-write-wins loses legitimate update  
335. Vector clock overflow  
336. Vector clock truncated losing causality  
337. Lamport clock skew across nodes  
338. Hybrid logical clock not synchronized  
339. Causality violation: effect visible before cause  
340. Read-your-writes broken after failover  
341. Monotonic reads violated  
342. Read committed but not yet replicated  
343. Replica freshness below staleness bound  
344. Eventual consistency never converges  
345. Anti-entropy repair never scheduled  
346. Merkle tree comparison too expensive  
347. Merkle tree mismatch not resolved  
348. Read repair writing wrong value  
349. Hinted handoff never delivered  
350. Hinted handoff delivered too late  
351. Read repair storm during recovery  
352. Read repair amplifying load on failing node  
353. Read repair using stale replica as source  
354. Read repair creating conflicting versions  
355. Gossip protocol not converging  
356. Gossip protocol spreading corrupted state  
357. Gossip protocol message loss causing state drift  
358. Epidemic broadcast partition isolated  
359. SWIM suspicion timeout too short  
360. SWIM false positive marking healthy node dead  

---

## Layer 13: Conflict Resolution & Merge (361–390)

361. SWIM false negative marking dead node alive  
362. Phi accrual failure detector miscalibrated  
363. Heartbeat lost on congested link  
364. Heartbeat delayed by GC pause  
365. Failure detector marking node dead during GC  
366. Failure detector not marking node dead after crash  
367. Accusation message lost  
368. Confirmation message lost  
369. Failure detector oscillation marking node up/down  
370. Membership list not converged  
371. Membership list has stale entries  
372. Node join not propagated to all  
373. Node leave not propagated to all  
374. Node leave while holding leases  
375. Lease renewal failing during network partition  
376. Lease expiry causing premature leadership loss  
377. Lease held too long after node death  
378. Fencing token not incremented  
379. Fencing token not checked by storage nodes  
380. Fencing token checked incorrectly  
381. STONITH not firing on failed node  
382. STONITH firing on healthy node  
383. Power controller unreachable for STONITH  
384. IPMI channel in use preventing STONITH  
385. Rebalance triggered during recovery  
386. Rebalance moving data away from recovering node  
387. Rebalance throttled too aggressively  
388. Rebalance causing thundering herd  
389. Data movement between nodes interrupted  
390. Data movement checksum verification skipped  

---

## Layer 14: Coordination & Leadership (391–420)

391. Data movement source deleted before destination verified  
392. Data movement destination full mid-transfer  
393. Data movement bandwidth not reserved  
394. Data movement priority not set  
395. Data movement competing with foreground traffic  
396. Replication factor temporarily below target  
397. Replication factor never restored after recovery  
398. Erasure code fragment missing  
399. Erasure code reconstruction failing  
400. Erasure code decode latency too high  
401. Erasure code fragment on same failure domain  
402. Cross-region replication not configured  
403. Cross-region replication lag unbounded  
404. Cross-region replication cost exceeding budget  
405. Nine-character operation reference collision  
406. Operation reference reused for different payload  
407. Operation reference not unique across regions  
408. Ordering position assigned out of sequence  
409. Ordering position gap never filled  
410. Ordering position reused after node restart  
411. Dataset-scoped ordering domain has no leader  
412. Dataset-scoped ordering domain leader unavailable  
413. Dataset-scoped ordering domain participants partitioned  
414. Cross-dataset transaction spanning ordering domains  
415. Cross-dataset transaction deadlock  
416. Cross-dataset transaction partial commit  
417. Cross-dataset transaction orphaned after coordinator crash  
418. Concurrent writes to same record no conflict rule  
419. Concurrent writes same record last-write-wins data loss  
420. Concurrent writes same record merge semantics undefined  

---

## Layer 15: Control Plane & Metadata (421–450)

421. Concurrent writes different records same partition serialized unnecessarily  
422. Read sees uncommitted write  
423. Read sees write from position 102 while 101 pending  
424. Read sees stale data after acknowledged write  
425. Write acknowledged but not durable  
426. Write committed locally but remote sync pending  
427. Write committed but durability condition not met  
428. Client retry after timeout causing duplicate write  
429. Idempotency key not checked  
430. Idempotency key checked but payload different  
431. Idempotency key storage lost on restart  
432. Idempotency key collision between unrelated operations  
433. Operation lifecycle state machine invalid transition  
434. Operation state lost on coordinator restart  
435. Operation state not persisted before ack  
436. Committed prefix gap: 101-103 committed, 104 missing, reader sees 105  
437. Ordered read blocked waiting for pending write  
438. Ordered read returns result as if pending write committed  
439. Causal dependency not declared by client  
440. Causal dependency declared but not enforced  
441. Dependency graph cycle causing deadlock  
442. Dependency graph unbounded growth  
443. Dependency pruning removing needed history  
444. Session affinity broken after node failover  
445. Session stickiness to failed node  
446. Session state not replicated  
447. Session token forged  
448. Session replay attack  
449. Read consistency profile changed mid-session  
450. Strong consistency requested but system returns eventual  

---

## Layer 16: Security & Authentication (451–480)

451. Eventual consistency requested but system blocks for strong  
452. Bounded staleness exceeded during regional outage  
453. Read from replica serving stale data beyond bound  
454. Write quorum not met but ack returned  
455. Read quorum not met but data returned  
456. Quorum intersection not guaranteed across failure domains  
457. Read repair done synchronously in read path  
458. Read repair latency added to read latency  
459. Read repair using wrong version as source  
460. Anti-entropy repair during peak traffic  
461. Anti-entropy repair not repairing all partitions  
462. Anti-entropy repair creating new conflicts  
463. Read-your-writes consistency not maintained across regions  
464. Monotonic read consistency violated after failover  
465. Prefix consistency violated  
466. Causal consistency violated  
467. Sequential consistency violated  
468. Linearizability violated  
469. Serializability violated  
470. Snapshot isolation violation  
471. Serializable snapshot isolation violation  
472. Write skew anomaly  
473. Read skew anomaly  
474. Phantom read anomaly  
475. Lost update anomaly  
476. Dirty read anomaly  
477. Non-repeatable read anomaly  
478. Serialization failure not retried by client  
479. Serialization failure retried but causes livelock  
480. Deadlock not detected  

---

## Layer 17: Authorization & Access Control (481–510)

481. Deadlock detection too slow  
482. Lock timeout too short causing false abort  
483. Lock timeout too long causing blocked transactions  
484. Lock not released after crash  
485. Lock table corruption  
486. Dead letter queue for unresolved conflicts not monitored  
487. Conflict resolution by human intervention not available  
488. Conflict resolution policy not versioned  
489. Conflict resolution policy changed mid-stream  
490. Application-level merge function bug  
491. CRDT state divergence  
492. CRDT merge not commutative  
493. CRDT merge not associative  
494. CRDT merge not idempotent  
495. Operation-based CRDT message lost  
496. Operation-based CRDT message duplicated  
497. Operation-based CRDT message reordered  
498. State-based CRDT state too large to transmit  
499. Delta-CRDT delta lost  
500. Delta-CRDT delta applied out of order  
501. Event sourcing event store corrupted  
502. Event replay producing different result  
503. Snapshot and event log inconsistent  
504. Projection not updated after event replay  
505. No permanent global master but domain leader election fails  
506. Leader election split vote  
507. Leader election tie not broken  
508. Leader election too slow  
509. Leader election too frequent (flapping)  
510. Leader not stepping down on partition  

---

## Layer 18: Operational & Human (511–540)

511. New leader not fencing old leader  
512. Old leader still accepting writes after replacement  
513. Two leaders in different partitions both accepting writes  
514. Leader partitioned from clients but still leader  
515. Leader reachable by some clients but not others  
516. Leader overwhelmed by request volume  
517. Leader CPU saturated  
518. Leader memory saturated  
519. Leader disk saturated by log writes  
520. Leader network saturated by replication  
521. Leader election quorum not reachable  
522. Leader term epoch not incremented  
523. Stale leader serving reads after epoch change  
524. Control plane and data plane disagree on current leader  
525. Control plane unavailable but data plane continues serving stale data  
526. Control plane partition isolating leader from followers  
527. Metadata cache stale after topology change  
528. Metadata cache invalidation missed  
529. Placement directory points to decommissioned node  
530. Placement directory unavailable during write  
531. Region join not reflected in global metadata  
532. Region leave not reflected in global metadata  
533. H-unit membership change not propagated  
534. Node capacity advertisement stale  
535. Node health check flapping  
536. Node health check false negative  
537. Node health check false positive  
538. Operator misconfiguration of replication policy  
539. Operator accidental deletion of dataset  
540. Operator granting excessive permissions  

---

## Layer 19: Cascade & Correlated Failures (541–570)

541. Operator failing to revoke credentials after employee departure  
542. Configuration push causing cascading restart  
543. Binary push introducing bug under load  
544. Capacity planning failure — region runs out of disk  
545. Capacity planning failure — region runs out of bandwidth  
546. Thundering herd on recovery after outage  
547. Retry storm amplifying load on failing node  
548. Cascading failure from single node overload  
549. Correlated failure — same rack loses power and network  
550. Correlated failure — same region loses power and network  
551. Correlated failure — same manufacturer disks fail together  
552. Correlated failure — same firmware bug triggers on all nodes  
553. Correlated failure — same timezone triggers batch job spike  
554. Correlated failure — DNS outage makes all regions unreachable  
555. Correlated failure — NTP outage breaks ordering across nodes  
556. Correlated failure — leap second causes widespread crash  
557. Correlated failure — daylight saving time transition breaks logs  
558. Correlated failure — SSL certificate expiry across fleet  
559. Correlated failure — dependency library vulnerability  
560. Correlated failure — language runtime garbage collection pause across nodes  
561. Correlated failure — kernel update requiring reboot of all nodes  
562. Correlated failure — security patch breaking compatibility  
563. Correlated failure — rolling upgrade leaving mixed versions  
564. Correlated failure — schema migration not backward compatible  
565. Correlated failure — feature flag enabling bad code path globally  
566. Correlated failure — load balancer health check marking all nodes unhealthy  
567. Correlated failure — autoscaling group spinning up thousands of nodes  
568. Correlated failure — autoscaling group terminating all nodes  
569. Correlated failure — cloud provider region-wide outage  
570. Correlated failure — cloud provider availability zone failure  

---

## Layer 20: Byzantine & Adversarial (571–600)

571. Node lies about its identity during HX-241 handshake  
572. Rotating network-facing identifier mapped to wrong internal node  
573. Cryptographic key rotated but old key still accepted for replay  
574. Byzantine node returns different data to different readers  
575. Malicious node corrupts erasure fragment and distributes it  
576. Replay attack using captured operation reference after commit  
577. Unauthorized region joins federation with forged credentials  
578. Data encryption key lost, making all replicas permanently unreadable  
579. Audit log tampered after the fact to hide unauthorized deletion  
580. Identity rotation during WAN partition leaves old and new identity both active  
581. Node claims capacity it does not have  
582. Node reports false health to avoid removal  
583. Node drops packets selectively to partition specific peers  
584. Node reorders packets to break ordering assumptions  
585. Node duplicates messages to cause idempotency failures  
586. Node injects spurious writes with forged references  
587. Node modifies data in transit between replicas  
588. Node suppresses acknowledgments to trigger retries  
589. Node impersonates another node's address  
590. Node colludes with other Byzantine nodes to form quorum  
591. Node selectively reveals different versions to different regions  
592. Node delays messages to violate staleness bounds  
593. Node fabricates checksums for corrupted data  
594. Node refuses to participate in repair while claiming health  
595. Node exfiltrates data through side channels  
596. Node performs timing attack to infer data values  
597. Node performs traffic analysis on metadata  
598. Node performs replay of old topology advertisements  
599. Node performs eclipse attack isolating a victim node  
600. Node performs sybil attack with multiple fake identities  

---

**End of domain inventory.**  
Total: 600 domains across 20 layers.  
Each domain is a category where IDS must define detection, isolation, and recovery behavior before deployment.