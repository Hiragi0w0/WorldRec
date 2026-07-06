import {
  listLibraryWorlds,
  type LibrarySearchCriteria,
  type LibrarySortDirection,
  type LibrarySortKey,
  type LibraryWorld,
} from "../api/commands";

export class LibraryState {
  private requestSeq = 0;

  items = $state<LibraryWorld[]>([]);
  totalCount = $state(0);
  limit = $state<10 | 25>(25);
  offset = $state(0);
  query = $state("");
  visitedFrom = $state("");
  visitedTo = $state("");
  tagQuery = $state("");
  memoQuery = $state("");
  sortKey = $state<LibrarySortKey>("world_name");
  sortDirection = $state<LibrarySortDirection>("asc");
  isLoading = $state(false);
  error = $state<string | null>(null);

  async loadLibrary() {
    const requestSeq = ++this.requestSeq;
    this.isLoading = true;
    this.error = null;

    try {
      const page = await listLibraryWorlds(this.buildCriteria());
      if (requestSeq === this.requestSeq) {
        this.items = page.items;
        this.totalCount = page.total_count;
        this.limit = normalizeLimit(page.limit);
        this.offset = Math.max(0, page.offset);
      }
    } catch (error) {
      if (requestSeq === this.requestSeq) {
        this.error = toErrorMessage(error);
        this.items = [];
        this.totalCount = 0;
      }
    } finally {
      if (requestSeq === this.requestSeq) {
        this.isLoading = false;
      }
    }
  }

  async goToPageAndLoad(page: number) {
    const totalPages = Math.max(1, Math.ceil(this.totalCount / this.limit));
    const safePage = Math.min(totalPages, Math.max(1, Math.floor(page)));
    this.offset = (safePage - 1) * this.limit;
    await this.loadLibrary();
  }

  async applySearchCriteria(criteria: {
    query: string;
    visitedFrom: string;
    visitedTo: string;
    tagQuery: string;
    memoQuery: string;
  }) {
    this.query = criteria.query;
    this.visitedFrom = criteria.visitedFrom;
    this.visitedTo = criteria.visitedTo;
    this.tagQuery = criteria.tagQuery;
    this.memoQuery = criteria.memoQuery;
    this.resetPaging();
    await this.loadLibrary();
  }

  async applySort(key: LibrarySortKey, direction: LibrarySortDirection) {
    this.sortKey = key;
    this.sortDirection = direction;
    this.resetPaging();
    await this.loadLibrary();
  }

  async applyLimit(limit: 10 | 25) {
    this.limit = limit;
    this.resetPaging();
    await this.loadLibrary();
  }

  async clearSearchCriteriaAndLoad() {
    this.query = "";
    this.visitedFrom = "";
    this.visitedTo = "";
    this.tagQuery = "";
    this.memoQuery = "";
    this.resetPaging();
    await this.loadLibrary();
  }

  private buildCriteria(): LibrarySearchCriteria {
    return {
      query: optionalString(this.query),
      visited_from: optionalString(this.visitedFrom),
      visited_to: optionalString(this.visitedTo),
      tags: parseTags(this.tagQuery),
      memo_query: optionalString(this.memoQuery),
      sort_key: this.sortKey,
      sort_direction: this.sortDirection,
      limit: this.limit,
      offset: this.offset,
    };
  }

  private resetPaging() {
    this.offset = 0;
  }
}

export function createLibraryState() {
  return new LibraryState();
}

function parseTags(value: string): string[] {
  return value
    .split(/[,\u3001\s]+/)
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);
}

function optionalString(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function normalizeLimit(limit: number): 10 | 25 {
  return limit === 10 ? 10 : 25;
}

function toErrorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Tauri command failed.";
}
