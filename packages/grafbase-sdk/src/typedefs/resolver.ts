import { AuthRuleF } from '../auth'
import { AuthDefinition } from './auth'
import { CacheDefinition, FieldCacheParams, FieldLevelCache } from './cache'
import { DefaultDefinition } from './default'
import { ReferenceDefinition } from './reference'
import { ScalarDefinition } from './scalar'
import { EnumDefinition } from './enum'
import { TagDefinition } from './tag'
import { InaccessibleDefinition } from './inaccessible'
import { ShareableDefinition } from './shareable'
import { OverrideDefinition } from './override'
import { ProvidesDefinition } from './provides'
import { DeprecatedDefinition } from './deprecated'

/**
 * A list of field types that can hold a `@resolver` attribute.
 */
export type Resolvable =
  | ScalarDefinition
  | DefaultDefinition
  | ReferenceDefinition
  | CacheDefinition
  | EnumDefinition<any, any>
  | TagDefinition
  | InaccessibleDefinition
  | ShareableDefinition
  | OverrideDefinition
  | ProvidesDefinition
  | DeprecatedDefinition

export class ResolverDefinition {
  private field: Resolvable
  private resolver: string
  private requiresFields: string | null

  constructor(field: Resolvable, resolver: string) {
    this.field = field
    this.resolver = resolver
    this.requiresFields = null
  }

  /**
   * Set the field-level auth directive.
   *
   * @param rules - A closure to build the authentication rules.
   */
  public auth(rules: AuthRuleF): AuthDefinition {
    return new AuthDefinition(this, rules)
  }

  /**
   * Set the field-level cache directive.
   *
   * @param params - The cache definition parameters.
   */
  public cache(params: FieldCacheParams): CacheDefinition {
    return new CacheDefinition(this, new FieldLevelCache(params))
  }

  /**
   * Declares that this resolver requires certain fields to function correctly
   *
   * @param fields - The fields this resolver requires
   */
  public requires(fields: string): ResolverDefinition {
    this.requiresFields = fields
    return this
  }

  public toString(): string {
    const requires =
      this.requiresFields == null
        ? ''
        : ` @requires(fields: "${this.requiresFields}")`

    return `${this.field} @resolver(name: "${this.resolver}")${requires}`
  }
}
