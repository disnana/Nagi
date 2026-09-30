
target/release/examples/microbench:     file format elf64-x86-64


Disassembly of section .text:

000000000007eea0 <microbench::sum>:
   7eea0:	48 85 f6             	test   %rsi,%rsi
   7eea3:	74 19                	je     7eebe <microbench::sum+0x1e>
   7eea5:	48 8d 14 f5 00 00 00 	lea    0x0(,%rsi,8),%rdx
   7eeac:	00 
   7eead:	48 83 c2 f8          	add    $0xfffffffffffffff8,%rdx
   7eeb1:	48 83 fa 18          	cmp    $0x18,%rdx
   7eeb5:	73 0a                	jae    7eec1 <microbench::sum+0x21>
   7eeb7:	31 c0                	xor    %eax,%eax
   7eeb9:	48 89 f9             	mov    %rdi,%rcx
   7eebc:	eb 55                	jmp    7ef13 <microbench::sum+0x73>
   7eebe:	31 c0                	xor    %eax,%eax
   7eec0:	c3                   	ret
   7eec1:	48 c1 ea 03          	shr    $0x3,%rdx
   7eec5:	48 ff c2             	inc    %rdx
   7eec8:	49 89 d0             	mov    %rdx,%r8
   7eecb:	49 83 e0 fc          	and    $0xfffffffffffffffc,%r8
   7eecf:	4a 8d 0c c7          	lea    (%rdi,%r8,8),%rcx
   7eed3:	66 0f ef c0          	pxor   %xmm0,%xmm0
   7eed7:	31 c0                	xor    %eax,%eax
   7eed9:	66 0f ef c9          	pxor   %xmm1,%xmm1
   7eedd:	0f 1f 00             	nopl   (%rax)
   7eee0:	f3 0f 6f 14 c7       	movdqu (%rdi,%rax,8),%xmm2
   7eee5:	66 0f d4 ca          	paddq  %xmm2,%xmm1
   7eee9:	f3 0f 6f 54 c7 10    	movdqu 0x10(%rdi,%rax,8),%xmm2
   7eeef:	66 0f d4 c2          	paddq  %xmm2,%xmm0
   7eef3:	48 83 c0 04          	add    $0x4,%rax
   7eef7:	49 39 c0             	cmp    %rax,%r8
   7eefa:	75 e4                	jne    7eee0 <microbench::sum+0x40>
   7eefc:	66 0f d4 c1          	paddq  %xmm1,%xmm0
   7ef00:	66 0f 70 c8 ee       	pshufd $0xee,%xmm0,%xmm1
   7ef05:	66 0f d4 c8          	paddq  %xmm0,%xmm1
   7ef09:	66 48 0f 7e c8       	movq   %xmm1,%rax
   7ef0e:	4c 39 c2             	cmp    %r8,%rdx
   7ef11:	74 19                	je     7ef2c <microbench::sum+0x8c>
   7ef13:	48 8d 14 f7          	lea    (%rdi,%rsi,8),%rdx
   7ef17:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
   7ef1e:	00 00 
   7ef20:	48 03 01             	add    (%rcx),%rax
   7ef23:	48 83 c1 08          	add    $0x8,%rcx
   7ef27:	48 39 d1             	cmp    %rdx,%rcx
   7ef2a:	75 f4                	jne    7ef20 <microbench::sum+0x80>
   7ef2c:	c3                   	ret
