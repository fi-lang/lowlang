#include "stdbool.h"
#include "stddef.h"

#ifndef include_runtime_metadata
#define include_runtime_metadata

typedef struct OpaqueS Opaque;
typedef struct TypeS Type;
typedef struct ValueWitnessTableS ValueWitnessTable;

typedef struct TypeS {
  ValueWitnessTable *vwt;
  size_t flags;
} Type;

#define TYPE_FLAG_TRIVIAL 1 << 0

typedef struct ValueWitnessTableS {
  size_t size;
  size_t align;
  size_t stride;
  void (*copy)(Opaque *dst, Opaque *src, Type *t);
  void (*move)(Opaque *dst, Opaque *src, Type *t);
  void (*drop)(Opaque *val, Type *t);
} ValueWitnessTable;

extern ValueWitnessTable OWNED_BOX_VWT;
extern ValueWitnessTable UNOWNED_BOX_VWT;
extern ValueWitnessTable RC_BOX_VWT;
extern ValueWitnessTable TRIVIAL_VWT[6];
extern Type TRIVIAL_METAS[6];

void copy_trivial(Opaque *dst, Opaque *src, Type *t);
void move_trivial(Opaque *dst, Opaque *src, Type *t);
void copy_move_nop(Opaque *dst, Opaque *src, Type *t);
void drop_nop(Opaque *val, Type *t);

#endif