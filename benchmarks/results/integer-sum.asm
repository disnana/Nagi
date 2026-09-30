
native-target/release/nagi-cpu:     file format elf64-x86-64


Disassembly of section .text:

0000000000024750 <nagi_cpu::integer_sum>:
   24750:	48 85 f6             	test   %rsi,%rsi
   24753:	74 19                	je     2476e <nagi_cpu::integer_sum+0x1e>
   24755:	48 8d 14 f5 00 00 00 	lea    0x0(,%rsi,8),%rdx
   2475c:	00 
   2475d:	48 83 c2 f8          	add    $0xfffffffffffffff8,%rdx
   24761:	48 83 fa 18          	cmp    $0x18,%rdx
   24765:	73 0a                	jae    24771 <nagi_cpu::integer_sum+0x21>
   24767:	31 c0                	xor    %eax,%eax
   24769:	48 89 f9             	mov    %rdi,%rcx
   2476c:	eb 55                	jmp    247c3 <nagi_cpu::integer_sum+0x73>
   2476e:	31 c0                	xor    %eax,%eax
   24770:	c3                   	ret
   24771:	48 c1 ea 03          	shr    $0x3,%rdx
   24775:	48 ff c2             	inc    %rdx
   24778:	49 89 d0             	mov    %rdx,%r8
   2477b:	49 83 e0 fc          	and    $0xfffffffffffffffc,%r8
   2477f:	4a 8d 0c c7          	lea    (%rdi,%r8,8),%rcx
   24783:	66 0f ef c0          	pxor   %xmm0,%xmm0
   24787:	31 c0                	xor    %eax,%eax
   24789:	66 0f ef c9          	pxor   %xmm1,%xmm1
   2478d:	0f 1f 00             	nopl   (%rax)
   24790:	f3 0f 6f 14 c7       	movdqu (%rdi,%rax,8),%xmm2
   24795:	66 0f d4 ca          	paddq  %xmm2,%xmm1
   24799:	f3 0f 6f 54 c7 10    	movdqu 0x10(%rdi,%rax,8),%xmm2
   2479f:	66 0f d4 c2          	paddq  %xmm2,%xmm0
   247a3:	48 83 c0 04          	add    $0x4,%rax
   247a7:	49 39 c0             	cmp    %rax,%r8
   247aa:	75 e4                	jne    24790 <nagi_cpu::integer_sum+0x40>
   247ac:	66 0f d4 c1          	paddq  %xmm1,%xmm0
   247b0:	66 0f 70 c8 ee       	pshufd $0xee,%xmm0,%xmm1
   247b5:	66 0f d4 c8          	paddq  %xmm0,%xmm1
   247b9:	66 48 0f 7e c8       	movq   %xmm1,%rax
   247be:	4c 39 c2             	cmp    %r8,%rdx
   247c1:	74 19                	je     247dc <nagi_cpu::integer_sum+0x8c>
   247c3:	48 8d 14 f7          	lea    (%rdi,%rsi,8),%rdx
   247c7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
   247ce:	00 00 
   247d0:	48 03 01             	add    (%rcx),%rax
   247d3:	48 83 c1 08          	add    $0x8,%rcx
   247d7:	48 39 d1             	cmp    %rdx,%rcx
   247da:	75 f4                	jne    247d0 <nagi_cpu::integer_sum+0x80>
   247dc:	c3                   	ret
