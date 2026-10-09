/* Stable Rust cannot define C variadic functions. Marshal only; filtering is Rust. */
#include <stdarg.h>
#include <stdlib.h>
#include "iksemel.h"
struct criterion { int kind; int number; const char *value; };
extern iksrule *iksemel_filter_add_rule_v(iksfilter *, iksFilterHook *, void *, const struct criterion *, size_t);
iksrule *iks_filter_add_rule(iksfilter *f, iksFilterHook *hook, void *user, ...) {
    struct criterion *items = NULL;
    size_t count = 0, capacity = 0;
    va_list ap;
    va_start(ap, user);
    for (;;) {
        int kind = va_arg(ap, int);
        if (kind == IKS_RULE_DONE) break;
        if (kind != IKS_RULE_TYPE && kind != IKS_RULE_SUBTYPE && kind != IKS_RULE_ID && kind != IKS_RULE_FROM && kind != IKS_RULE_FROM_PARTIAL && kind != IKS_RULE_NS) { free(items); va_end(ap); return NULL; }
        if (count == capacity) {
            capacity = capacity ? capacity * 2 : 8;
            void *next = realloc(items, capacity * sizeof(*items));
            if (!next) { free(items); va_end(ap); return NULL; }
            items = next;
        }
        items[count].kind = kind; items[count].number = 0; items[count].value = NULL;
        if (kind == IKS_RULE_TYPE || kind == IKS_RULE_SUBTYPE) items[count].number = va_arg(ap, int);
        else items[count].value = va_arg(ap, const char *);
        count++;
    }
    va_end(ap);
    /* Rust slice construction requires a non-null pointer even for zero length. */
    struct criterion empty = {0,0,NULL};
    iksrule *result = iksemel_filter_add_rule_v(f, hook, user, count ? items : &empty, count);
    free(items); return result;
}
