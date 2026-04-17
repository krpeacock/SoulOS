The history of mobile computing is often viewed through a lens of linear progression, where each successive generation of hardware simply provides more processing power and memory than the last. However, the early era of the PalmOS (1996–2003) represents a distinct architectural and philosophical detour. During this period, developers and designers were forced to contend with hardware so constrained that traditional desktop computing paradigms were not merely inefficient but entirely non-functional. This environment gave birth to the Zen of Palm, a design philosophy rooted in radical simplification, and a parallel technical movement aimed at creating "self-hosting" development environments. These frameworks, most notably those written in the languages they themselves implemented, such as Squeak Smalltalk and various Forth and C implementations, attempted to transform the handheld from a passive "paper replacement" into a sovereign, self-reproducing workspace.

## **The Lineage of Pen Computing: From GridPad to MobileBuilder**

The architectural DNA of the PalmOS was not created in a vacuum; it was the culmination of over a decade of experimentation with pen-based computing that began with the GRID Systems GridPad. This lineage represents the transition from heavy, DOS-based tablets to the streamlined "Zen" of the PalmPilot.

### **The Evolution of GRiDPen and PenRight\!**

The first commercially successful tablet computer, the GridPad 1900 (1989), was developed under the leadership of Jeff Hawkins. To solve the problem of interacting with an MS-DOS system without a keyboard, Hawkins developed **GRiDPen**, a software suite that provided the first robust handwriting recognition engine for the platform. GRiDPen was later licensed and evolved into **PenRight\!** (sometimes referred to in its early stages as PenWrite), which became a foundational environment for pen-based applications throughout the early 1990s.

PenRight\! was designed to bridge the gap between traditional text-based DOS and the needs of mobile, pen-driven interfaces. It was eventually ported to Windows and later to the PalmOS, where it was rebranded and marketed as **MobileBuilder**. This evolution was characterized by several key technical shifts:

- **Event-Driven Architecture:** Unlike standard DOS programs that followed a linear execution path, PenRight\! implemented an event-driven model. Every tap of the stylus or release of the pen triggered specific events that the application had to intercept and process, a paradigm that would become standard in the PalmOS.
- **Overlapped Window Managers:** PenRight\! Pro introduced a sophisticated window manager that supported overlapped windows and sculpted, 3D-style visual components reminiscent of the Motif graphical interface. This was a radical departure for the era, as it provided a "multi-windowed world" on devices that often had less-than-386 performance.
- **Rapid Application Development (RAD):** The lineage culminated in MobileBuilder, a C-based RAD environment that allowed developers to lay out "forms" graphically using tools like DesignForm and PenResource. This allowed for "cross-platform" development, where a single set of code could target MS-DOS, Windows CE, and PalmOS.

## **The Zen of Palm: Minimalism as a Technical Imperative**

The design philosophy of early PalmOS was a direct response to the physical limitations of the hardware. The original Pilot 1000 featured a Motorola 68328 "DragonBall" processor running at a mere 16 MHz, with a 160x160 monochrome display and only 128 KB of RAM.1 In such an environment, the cognitive load of a complex interface would be compounded by the latency of the system’s response. Thus, the Zen of Palm emerged as a discipline of ruthless prioritization.

### **The 1-2-3 Rule and the Paradox of the Heap**

At the heart of Palm’s design methodology was the 1-2-3 Rule, a framework for feature selection and interface layout. The rule mandated three distinct steps: identifying the specific problems a user faced, finding the simplest possible solution to each problem, and, most crucially, getting rid of everything else. This final step—the elimination of non-essential features—was considered the most difficult to implement but was the primary factor in the platform’s success. Designers were encouraged to view their applications not as a "heap" of features but as a focused tool. Drawing from the philosophical paradox of the heap, the Zen of Palm suggested that an application remains a "heap" of utility only as long as its core grains of functionality are not diluted by peripheral requirements.

To determine what constituted a core requirement, Palm advocated for the 80/20 Rule. This principle posited that the tasks users spend 80% of their time performing represent the core need of the application. Any feature that fell into the remaining 20% of usage time was a candidate for removal or concealment. This led to an interface style characterized by immediate accessibility, where the most common tasks were reachable within one or two taps, and "power features" were included only discreetly for advanced users.

### **The Paper Replacement Metaphor and User Persistence**

The PalmOS was intended to be a "paper replacement," a metaphor that dictated the system's "instant-on" capability and its lack of a traditional save-and-load cycle. Because the RAM was physically the same as the storage medium, applications were executed in place (XIP), meaning they did not need to be loaded into a separate "working memory".1 This architecture fostered a sense of persistence; when a user switched applications, the state was preserved exactly as it was, mirroring the experience of flipping through a physical notebook. This persistence was not just a convenience but a core design practice aimed at making the device intuitive and easy to remember.

| PalmOS Version | CPU Frequency  | Display Characteristics | Significant Features                          |
| :------------- | :------------- | :---------------------- | :-------------------------------------------- |
| Palm OS 1.0    | 16 MHz (68k)   | 160x160 Monochrome      | No file system; RAM-only storage; Graffiti 1  |
| Palm OS 3.0    | 16-20 MHz      | 160x160 2-bit Gray      | Infrared support; larger application limits 1 |
| Palm OS 3.5    | 20 MHz         | 8-bit Color Support     | Context-sensitive icon bar; agenda view 1     |
| Palm OS 5.0    | 200+ MHz (ARM) | 320x320/480x320         | PACE 68k emulation; high resolution 1         |

## **Technical Constraints and the System-as-Database Architecture**

The technical underpinnings of PalmOS were defined by an almost total absence of standard operating system abstractions. There was no file system in the traditional sense; instead, the system utilized a database-centric storage model.1 Every piece of data, from a memo to the application code itself, was stored as a record in a Palm Database (PDB) or Palm Resource (PRC) file.2

### **Memory Segmentation and the 64 KB Barrier**

The memory architecture of the Motorola 68k processor family used by early Palm devices imposed a severe restriction: contiguous data blocks and code segments could not exceed 64 KB.2 This "64 KB barrier" profoundly influenced the development of on-device frameworks. A compiler or interpreter had to be designed to fit its executable code into these small segments while also managing a dynamic heap that was often as small as 12 KB to 96 KB.2 For developers accustomed to the "everything is a file" philosophy of Unix, the PalmOS was an alien landscape where "files" did not exist and standard I/O (STDIN/STDOUT) had no meaning.4

### **Execution in Place and Write Protection**

The lack of a dedicated file system meant that applications were installed directly into RAM and executed there. To protect the integrity of the system, the PalmOS utilized a hardware write-protection mechanism for the storage heap.2 Modifying a database record required special system calls that would briefly disable write-protection to commit changes.2 This created a performance bottleneck for any framework attempting to provide a traditional "writable" environment, such as a self-hosting compiler or an interactive Lisp environment. Frameworks like LispMe and OnBoard C had to find innovative ways to bypass or optimize these system traps to maintain responsiveness.2

## **Self-Hosting Frameworks: The Quest for Autonomy**

A self-hosting environment is one in which the tools used to create software are themselves created using those same tools. In the context of the early PalmOS, this was a radical goal. Most development was done via cross-compilation on powerful desktop workstations using Metrowerks CodeWarrior or the GCC-based PRC-Tools.8 However, a community of "builders" sought to make the Palm a self-sufficient platform where code could be written, compiled, and executed entirely on the handheld.7

### **Squeak Smalltalk: Back to the Future**

The most prominent example of a framework "written in itself" on the Palm platform (and its progenitors like the Itsy pocket computer) was Squeak Smalltalk. Squeak was designed with the explicit goal of writing as much code as possible in Smalltalk itself.12 This included the compiler, the GUI framework, and even the core graphics primitives like BitBlt, which were written in a subset of Smalltalk and then translated into C for the virtual machine’s core.14

Squeak’s port to the Palm and Itsy environments represented a technical tour de force. Because Squeak applications were developed as an "image"—a serialized snapshot of the entire object memory—they could be transferred between the desktop and the handheld seamlessly.12 The Squeak VM on the handheld provided an execution environment for these images, allowing the Palm to run a full-blown, object-oriented development environment that was, in a very literal sense, "back to the future": a practical Smalltalk written in itself.14

### **OnBoard C: The Handheld Workspace**

While Squeak provided an object-oriented ideal, OnBoard C offered a more traditional, yet equally impressive, self-hosted environment for C programmers. Developed by Roger Lawrence of Individeo, OnBoard C was a full C compiler and Integrated Development Environment (IDE) that ran directly on the PalmOS.17

The OnBoard Suite consisted of several modular components designed to work within the system's constraints:

- **OnBoard C Compiler:** Translated C source into 68k assembly.17
- **OnBoard Assembler:** Processed assembly into native PRCs.17
- **SrcEdit:** A programmer's editor that integrated with the compiler.18
- **RsrcEdit:** A visual resource editor for designing GUI elements like forms and buttons.18

One of the key innovations of OnBoard C was its use of pre-compiled headers. To avoid the massive overhead of parsing the thousands of lines of PalmOS API declarations on every compile, OnBoard C used a compressed, binary database (CompilerData.OnBC) to populate its symbol table instantly upon launch.17 While early versions of the suite were cross-compiled, the community eventually aimed for a fully self-bootstrapping version, where OnBoard C could compile its own source code on the device.19

| Framework     | Implementation Language  | Execution Model         | Notable Characteristic                |
| :------------ | :----------------------- | :---------------------- | :------------------------------------ |
| Squeak        | Smalltalk (Self-written) | Virtual Machine / Image | Fully self-hosting environment 12     |
| OnBoard C     | C (Self-hosted)          | Native 68k Compilation  | Uses pre-compiled header databases 17 |
| Quartus Forth | Forth (Self-hosted)      | Native 68k Compilation  | Interactive REPL and console 21       |
| LispMe        | C / SECD Bytecode        | SECD Virtual Machine    | Persistent sessions in databases 2    |

## **Quartus Forth and the Efficiency of the Stack**

Forth has historically been the language of choice for resource-constrained systems, and Quartus Forth was the primary implementation for the PalmOS. Forth is inherently suited to self-hosting because its compiler is extremely simple, often consisting of just a few hundred lines of code that can easily fit within a 64 KB segment.

### **Interactive Compilation and the Console**

Quartus Forth allowed developers to write code in the built-in Memo Pad and compile it instantly into native machine code PRCs.21 Unlike C, which required a multi-stage edit-compile-run cycle, Forth provided an interactive console. This enabled a developer to test individual "words" (functions) in real-time on the hardware, providing a level of feedback that was otherwise impossible on the Palm platform.21

The Quartus environment also integrated the RsrcEdit tool, allowing for a complete GUI development workflow on the device. Because Forth is a stack-based language, it could call the \~900 PalmOS system routines directly by pushing arguments onto the data stack, making it a highly efficient way to access the underlying OS without the overhead of a heavy runtime library.21

## **LispMe: Functional Persistence on the SECD Machine**

LispMe, a Scheme implementation for the PalmOS, brought the power of functional programming and first-class continuations to the handheld.2 It was based on the SECD (Stack, Environment, Code, Dump) virtual machine, a classic model for Lisp implementation.2

### **Architectural Adaptations for Database Storage**

LispMe's implementation of the SECD machine was specifically modified to handle the Palm's lack of a flat memory space. It used 16-bit relative pointers within its heap, which allowed the entire Scheme object memory to be stored as a single, relocatable database record.2 This allowed for "persistent sessions": a user could be in the middle of a complex calculation, switch to the Datebook to check an appointment, and return to LispMe to find the VM state exactly as they left it.2

Key technical features of LispMe included:

- **Variable-Arity Support:** Using negative arity values to handle functions with optional or rest arguments.4
- **Tail Recursion Optimization:** Instructions like TAPC and SELR ensured that recursive calls did not exhaust the limited 2 KB system stack.2
- **Global Variable Access:** Optimized to O(1) by embedding storage cell references directly into the bytecode.4
- **Break Monitoring:** To prevent a long-running Lisp process from "hanging" the single-tasking OS, the VM checked the event queue every 1600 steps for a user-initiated break signal.2

## **RAD Tools and the Visual Basic Paradigm**

While self-hosting frameworks catered to the "builder" archetype, other tools sought to bring the Rapid Application Development (RAD) experience of the desktop to the Palm. These tools, such as CASL and PocketStudio, were often cross-compilers but provided high-level abstractions that hid the complexities of the PalmOS API.

### **CASL and Cross-Platform Portability**

The Compact Application Solution Language (CASL) was an event-driven language similar to BASIC or Pascal.24 Its primary value proposition was "write once, run all," allowing developers to target PalmOS, Windows, and PocketPC from a single codebase.24 The CASL IDE featured a visual form editor where objects were positioned and then linked to event-driven code.26 For the PalmOS target, CASL would translate its high-level code into C, which was then compiled into a native PRC using the open-source GCC toolchain.26

### **PocketStudio and the Pascal Legacy**

PocketStudio brought the Delphi/Object Pascal experience to the Palm, featuring a visual form designer and an object inspector.5 It targeted the Motorola 68k instruction set directly, allowing for efficient native code generation while providing the safety and structure of the Pascal language.27 These tools were essential for the "prosumer" market—developers who needed to build functional, professional-looking applications quickly but did not necessarily want to delve into the minutiae of 68k assembly or manual memory management.

## **Socio-Technical Reflections on the Early Handheld Era**

The early PalmOS ecosystem was defined by a tension between the "Zen" of the user experience and the technical "chaos" of the underlying hardware. The success of the platform was due to the designers' ability to reconcile these two through radical simplification.

### **The Disconnect Between Builders and Users**

A recurring theme in the discourse of the era was the disconnect between the technology industry and the common user. While techies viewed computers as things to be updated and replaced, the common user viewed them as tools that should "just work" for decades.11 The Zen of Palm was an attempt to bridge this gap by making the software so simple and reliable that it felt like an extension of the user's physical world.

### **The Legacy of Self-Hosting on Mobile**

The frameworks like Squeak, OnBoard C, and Quartus Forth represent a lost chapter in mobile history where the handheld was seen as a potential replacement for the workstation. The 80/20 Rule applied to these frameworks as well; while 80% of users only needed a few pre-packaged apps, the 20% who were power users or developers needed the platform to be open and self-sufficient. This ethos was eventually eclipsed by the "app store" model of later smartphones, where the development environment was strictly separated from the target device.

## **Conclusion**

The early PalmOS was a masterclass in the architecture of constraint. Through the Zen of Palm, designers learned to prioritize the essential, creating an interface that was both fast and intuitive. Simultaneously, the development of self-hosting frameworks like Squeak and OnBoard C proved that even the most limited hardware could be turned into a powerful, self-reproducing workspace. These frameworks were not merely technical achievements; they were expressions of a philosophy that valued autonomy, persistence, and the elegance of simplicity. As modern mobile operating systems grow increasingly complex, the lessons of the Palm era—the 1-2-3 Rule, the 64 KB segment as a unit of focus, and the ideal of the computer as a workshop—remain highly relevant for the next generation of computing.

#### **Works cited**

1. Palm OS \- Wikipedia, accessed April 17, 2026, [https://en.wikipedia.org/wiki/Palm_OS](https://en.wikipedia.org/wiki/Palm_OS)
2. LispMe: An Implementation of Scheme for the Palm Pilot, accessed April 17, 2026, [https://www.schemeworkshop.org/2001/bayer01lispme.pdf](https://www.schemeworkshop.org/2001/bayer01lispme.pdf)
3. Object Pascal \- Informatics Engineering | Wiki eduNitas.com, accessed April 17, 2026, [https://wiki.edunitas.com/IT/en/114-10/Object-Pascal_3284_Umb_eduNitas.html](https://wiki.edunitas.com/IT/en/114-10/Object-Pascal_3284_Umb_eduNitas.html)
4. LispMe: An Implementation of Scheme for the Palm Pilot \- 3e8.org, accessed April 17, 2026, [https://3e8.org/pub/LispMe.pdf](https://3e8.org/pub/LispMe.pdf)
5. Programming | Jim's Random Notes | Page 21, accessed April 17, 2026, [https://blog.mischel.com/category/programming/page/21/](https://blog.mischel.com/category/programming/page/21/)
6. Oo Environment For Palm \- C2 Wiki, accessed April 17, 2026, [https://wiki.c2.com/?OoEnvironmentForPalm](https://wiki.c2.com/?OoEnvironmentForPalm)
7. PalmOS Hosted Programming Languages: \-- Using the Palm as a Development Environment \-- \- Gnosis Software, accessed April 17, 2026, [https://gnosis.cx/publish/programming/palm_languages.html](https://gnosis.cx/publish/programming/palm_languages.html)
8. Palm Programming Companion CD \- PalmDB, accessed April 17, 2026, [https://palmdb.net/app/palm-programming-companion-cd](https://palmdb.net/app/palm-programming-companion-cd)
9. Proceedings of the LISA 2001 15 Systems Administration Conference \- USENIX, accessed April 17, 2026, [https://www.usenix.org/legacy/event/lisa2001/tech/full_papers/okay/okay.pdf](https://www.usenix.org/legacy/event/lisa2001/tech/full_papers/okay/okay.pdf)
10. Palm OS Programming: The Developer's Guide, Second Edition, accessed April 17, 2026, [https://api.pageplace.de/preview/DT0400.9781449369088_A24028241/preview-9781449369088_A24028241.pdf](https://api.pageplace.de/preview/DT0400.9781449369088_A24028241/preview-9781449369088_A24028241.pdf)
11. Lotus 1-2-3 \- Hacker News, accessed April 17, 2026, [https://news.ycombinator.com/item?id=35872758](https://news.ycombinator.com/item?id=35872758)
12. NXTalk: dynamic object-oriented programming in a constrained environment \- Hasso-Plattner-Institut, accessed April 17, 2026, [https://hpi.uni-potsdam.de/hirschfeld/publications/media/BeckHauptHirschfeld_2009_NXTalkDynamicObjectOrientedProgrammingInAConstrainedEnvironment_AcmDL.pdf](https://hpi.uni-potsdam.de/hirschfeld/publications/media/BeckHauptHirschfeld_2009_NXTalkDynamicObjectOrientedProgrammingInAConstrainedEnvironment_AcmDL.pdf)
13. Java™ on the bare metal of wireless sensor devices: the squawk Java virtual machine, accessed April 17, 2026, [https://www.researchgate.net/publication/234804740_Java_on_the_bare_metal_of_wireless_sensor_devices_the_squawk_Java_virtual_machine](https://www.researchgate.net/publication/234804740_Java_on_the_bare_metal_of_wireless_sensor_devices_the_squawk_Java_virtual_machine)
14. The Itsy Pocket Computer \- Waldspurger.org, accessed April 17, 2026, [https://www.waldspurger.org/carl/papers/itsy-wrl-20006.pdf](https://www.waldspurger.org/carl/papers/itsy-wrl-20006.pdf)
15. (PDF) The Itsy Pocket Computer \- ResearchGate, accessed April 17, 2026, [https://www.researchgate.net/publication/2432274_The_Itsy_Pocket_Computer](https://www.researchgate.net/publication/2432274_The_Itsy_Pocket_Computer)
16. Where »less« is »more« – notions of minimalism and the design of interactive systems: \- ediss.sub.hamburg, accessed April 17, 2026, [https://ediss.sub.uni-hamburg.de/bitstream/ediss/2132/1/obendorf.pdf](https://ediss.sub.uni-hamburg.de/bitstream/ediss/2132/1/obendorf.pdf)
17. The OnBoard Suite User's Guide \- SourceForge, accessed April 17, 2026, [https://onboardc.sourceforge.net/UsersManual.html](https://onboardc.sourceforge.net/UsersManual.html)
18. The OnBoard Suite \-- an integrated development environment for and \*on\* the Palm, accessed April 17, 2026, [https://onboardc.sourceforge.net/](https://onboardc.sourceforge.net/)
19. onboardc.sourceforge.net, accessed April 17, 2026, [https://onboardc.sourceforge.net/\_faq.html](https://onboardc.sourceforge.net/_faq.html)
20. OnBoard C \- TextEditors Wiki, accessed April 17, 2026, [https://texteditors.org/cgi-bin/wiki.pl?action=browse\&diff=1\&id=OnBoard_C\&revision=3](https://texteditors.org/cgi-bin/wiki.pl?action=browse&diff=1&id=OnBoard_C&revision=3)
21. Quartus Forth (Palm OS® Version), accessed April 17, 2026, [https://www.quartus.net/products/forth/](https://www.quartus.net/products/forth/)
22. Some Forth Systems to Try \- Fig UK, accessed April 17, 2026, [http://www.figuk.plus.com/4thres/systems.htm](http://www.figuk.plus.com/4thres/systems.htm)
23. LispMe \- Wikipedia, accessed April 17, 2026, [https://en.wikipedia.org/wiki/LispMe](https://en.wikipedia.org/wiki/LispMe)
24. Compact Application Solution Language \- Wikipedia, accessed April 17, 2026, [https://en.wikipedia.org/wiki/Compact_Application_Solution_Language](https://en.wikipedia.org/wiki/Compact_Application_Solution_Language)
25. CASL Home for Android and Windows, accessed April 17, 2026, [https://www.caslsoft.com/](https://www.caslsoft.com/)
26. Computer aided data acquisition tool for high-throughput phenotyping of plant populations, accessed April 17, 2026, [https://pmc.ncbi.nlm.nih.gov/articles/PMC2796657/](https://pmc.ncbi.nlm.nih.gov/articles/PMC2796657/)
27. List of compilers and interpreters | Pascal Wiki | Fandom, accessed April 17, 2026, [https://pascal.fandom.com/wiki/List_of_compilers_and_interpreters](https://pascal.fandom.com/wiki/List_of_compilers_and_interpreters)
